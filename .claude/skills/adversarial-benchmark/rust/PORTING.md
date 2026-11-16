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

## R10 데이터셋 전처리·행 선택(`dataset_prep`) 이행 (후보 · 정본 적용 전)

상태: **COMPLETE (이 변환 범위의 후보)** — 전체 과업·여섯 프로파일·실제 Long E2E 의 완료가 아니다. 대상은 원본 `dataset_prep.py`(286줄) 전부다: `cmd_prepare` · `cmd_slice` ·
`cmd_synthetic_sidecar` · `_load_spec` · `_dir_digest` · `target_rows` · `data_arg` · `column_mapper_arg` · `_die` · `main`(argparse 3개 서브명령 + `--self-test`) ·
`_self_test`(T1~T7). 같은 폴더의 `resolve_bench_tool`·`resolve_guidellm`·`resolve_topology` 는 범위 밖이다(아직 이행하지 않았고 연결도 없다).
원본(기준 `831068cb`)은 이행 대조 기준으로 보존하며 런타임 대체 경로(Python 호출)는 없다. 실제 `docker` 는 외부 런타임이며 크레이트가 흉내 내지 않는다.

### 인터페이스 델타

| 산출 | 내용 |
|---|---|
| CLI | `dataset_prep` — 원본 argparse(prog `dataset_prep.py` / `dataset_prep.py <서브명령>`, 서브명령 `prepare` `slice` `synthetic-sidecar`, 약어·`--opt=값`·`--`·오류 문구·도움말/사용법 폭 1..400 캡처·`--self-test` 는 단독 인자일 때만)·출력·종료코드(0/2/3, 처리되지 않은 예외 1)·실패 시점을 그대로 재현. 서브명령 단어 앞의 최상위 옵션, 서브명령 뒤의 미인식 인자(최상위 오류), 필수 누락이 미인식보다 먼저 보고되는 순서까지 원본과 같다. |
| 라이브러리 | `dataset_prep`(상수·`load_spec`·`dir_digest`·`column_mapper_arg`·`data_arg`·`target_rows`·줄 읽기) · `dataset_commands`(`cmd_*`, Namespace = 동적 타입 속성) · `dataset_random`(MT19937 + CPython `str` 시드) · `dataset_prep_cli` · `dataset_prep_selftest` · `dataset_prep_api` · 생성 데이터 `help_layouts_dataset_prep`. |
| 범용 API | `dataset_prep_api evaluate-jsonl` — 행 `{id, operation, arguments}`, `operation` 은 원본 함수명(`_load_spec` `_dir_digest` `column_mapper_arg` `data_arg` `target_rows` `cmd_prepare` `cmd_slice` `cmd_synthetic_sidecar` `main` `_self_test` `_die` `constants`). 결과 `{id,kind:value,value}` / `{id,kind:exception,type,message}` + (표준오류/표준출력을 썼을 때) `stderr`/`stdout`. `_load_spec` 의 거부는 `SystemExit` + `stderr`. `cmd_*` 는 `argparse.Namespace` 를 속성 객체로 받는다. `_dir_digest` 의 `d` 는 `pathlib.Path` 의 전송 표현(문자열)만 받는다(그 밖의 타입은 원본처럼 `iterdir` 없음 AttributeError). `arguments_json`/`result_json` 옵션과 `constants`(상수는 Rust 소유)·`native-help`. |
| 시간 측정 | `dataset_prep_bench ROWS.jsonl ROUNDS ITERATIONS [ID...]` — 같은 `evaluate_operation` 디스패치, 사례 지식 없음. 준비(행 파싱)는 밖; 디스패치·바인딩·본문·파일 읽기·해시·전체 값 생성/해제는 안; 시간 밖 워밍 1회가 `Ok`·무진단·`expected_json`(컴팩트 ASCII) 전체 일치를 요구하고 시간 안의 `Err`/진단은 종료 1 · 타이밍 없음. |
| 공유 런타임 편집(가산·가시성) | `pyvalue.rs` `dumps_ascii_tight`(`separators=(",",":")`) · `cli.rs` `on_large_stack` 공개 + `Streams::flush_if_terminal`(터미널일 때만 줄 단위 flush) · `argv.rs` 에 `Spec::with_layouts`·`parse_known_with`/`Known`/`unrecognized`/`is_option_like`(서브명령 분배용; `parse_with` 는 이를 감싸 동작 불변)와 `DATASET_*` 4개 `Spec` · `pyfs.rs` `read_in_blocks` · `lib.rs` 모듈 등록 · `Cargo.toml` bin 3 + `[[test]]` 7 + `rustix` 기능 `process`(getuid/getgid; 새 크레이트 없음, lock 불변) · `notices/`(MT19937 표기, 두 행 추가). |

### 원본 → Rust 대응

`_load_spec` → `dataset_prep::load_spec` · `_dir_digest` → `dir_digest` · `column_mapper_arg` · `data_arg` · `target_rows`(`TargetRows`) · `_die` → `die` · `cmd_prepare`/`cmd_slice`/`cmd_synthetic_sidecar` → `dataset_commands` ·
`main` → `dataset_prep_cli::main_with`(+`run`) · `_self_test` → `dataset_prep_selftest::self_test`(임시 폴더는 `TemporaryDirectory` 처럼 지운다) · `random.Random(str).randrange` → `dataset_random::PythonRandom::randbelow`.

### 보존한 특이 동작

* **캐시 키**: `source_sha256`·`dataset_kind`·`column_map`·`preprocess_strategy`·`seed`(스펙 값 그대로)·`input_len`·`output_len`·`tokenizer_digest`·`image`. 토크나이저 폴더는 매번 정렬된 파일 이름과 **각 파일 전체의 SHA-256** 으로 해시한다(심볼릭 링크는 따라가고 하위 폴더·끊긴 링크는 건너뛴다; 이름은 엄격 UTF-8 인코딩이라 비-UTF-8 이름은 `UnicodeEncodeError`). 캐시 적중은 메타 `key` 가 키와 `==`(NaN 은 디코더의 공유 객체 정체성)일 때만이며 이때 Docker 를 부르지 않는다. 불일치/읽기 불가(ValueError 계열)는 메타·산출물을 원본 순서(`mkdir -p` → 메타 → 산출물 `unlink(missing_ok)`)로 지운다. OSError 는 잡지 않는다.
* **Docker**: `--network none`·`--user uid:gid`·`--memory`/`--memory-swap`·`-e` 4개·`-v` 3개(`:ro`/`:ro`/쓰기)·`--entrypoint guidellm`·이미지·`preprocess dataset`·`data_arg`·`--tokenizer`·`--strategy`·`--data-column-mapper`·`--seed`(`int(spec["seed"])`) 순서 그대로; `finally` 의 `docker rm -f` 는 항상, 타임아웃이면 핸들러에서 한 번 더(총 두 번), `int(a.timeout_s)` 오류도 `finally` 를 거친다. 타임아웃은 `Popen._wait` 처럼 0.5ms 에서 50ms 로 늘어나는 폴링이며 초과 시 SIGKILL 후 회수. 자식은 부모의 입출력을 그대로 물려받는다 — **파이프에서는 자식의 표준출력이 Python 의 블록 버퍼링된 배너보다 먼저 나오고**(`StdoutModel`), 터미널에서는 `print` 마다 flush 해 배너가 먼저다(원본 PTY 관측과 같음). 자식 출력을 가리거나 잡지 않는다. `docker` 가 없으면 `FileNotFoundError: [Errno 2] No such file or directory: 'docker'` 가 `finally` 의 정리 호출에서 한 번 더 나며 그대로 전파된다.
* **줄 읽기**: `for line in f` 는 `read()` 와 다르다 — 8192바이트 원시 읽기 단위로 증분 UTF-8 해석하므로 나쁜 바이트는 **자기 chunk 만** 잃고 앞 chunk 의 완성된 줄은 먼저 처리된다(그 줄의 JSON 오류가 해석 오류보다 우선); 위치는 보류 중인 바이트 기준; chunk 끝의 `\r` 은 다음 chunk 까지 보류; `\r\n`/`\r` → `\n`; `line.strip()` 은 유니코드 공백. `read_text()`(스펙·메타)는 한 번에 해석한다.
* **slice**: 목표 길이는 정수 개수의 **최빈값**(동률이면 작은 값; `bool` 은 `int`, `True`/`1` 은 한 키이며 처음 철자가 남음; 키 정체성은 Python 과 같다)이고 `|target − input_len| > 8` 이면 빈 풀; 풀 선택의 등치 `c == target` 은 `1024.0`·`True` 도 포함한다. salt 는 `sha256(salt.encode())` 앞 16 hex 를 풀 크기로 나눈 나머지이며 연속 구간은 `mod len(pool)` 로 감긴다; 모자라면 `random.Random("%s:%d" % (seed, idx))` 의 `randrange` 중복 허용 추출(`dataset_random`: MT19937, 시드 = `str.encode()` + SHA-512 → 32비트 단어 `init_by_array`, `getrandbits` 단어 채움, 거부 표본). 출력 파일은 사이드카보다 먼저 쓰이고 사이드카 실패 시 남는다; 사이드카 딕셔너리 평가 순서(`int(a.output_len)` → `meta.get("rows")` → `a.tokenizer_label` → `meta["key"]["tokenizer_digest"]`)와 그 예외·부분 상태가 원본과 같다.
* **예외·종료**: `_die` → stderr `[dataset_prep] 메시지` + `SystemExit(rc)`; 처리되지 않은 예외는 추적 머리 표지 + `Class: message`(`JSONDecodeError` 는 `json.decoder.JSONDecodeError`)와 종료 1(Python 프레임은 재현하지 않음); `except (OSError, ValueError)`·`except ValueError`·`except TimeoutExpired` 의 포착 범위를 그대로 따른다(`RecursionError` 는 잡지 않음).
* **JSON 입력의 동적 타입**: 필드는 문서가 담은 아무 JSON 값이며 `Path(None)`/`Path(5)` TypeError 문구, `spec["x"]` 의 KeyError/TypeError 문구, `int()` 의 문구(공백·밑줄·유니코드 숫자·4300자리 한도), `%s`/`%d` 서식, `json.dumps` 의 `NaN`/`Infinity` 가 모두 원본과 같다.

### 알려진 한계·미연결(root)

(1) 런처·`build_rust.sh` 검사 등록·소비자(`run_bench.sh` 의 6곳) 배선·정본 적용은 root 단계다(초안 `drafts/`: CLI 런처 템플릿, API 런처, 전송 브리지 `dataset_prep_native.py`, 통합 노트). (2) 실제 Docker/GuideLLM/컨테이너 성공은 이 단위로 증명되지 않는다 — 검사는 명령·부수효과 고정물(PATH 의 가짜 `docker` 스크립트)만 쓴다. (3) 원본의 `--count 10000000000000000000000000` 같은 거대 값은 무작위 분기에서 끝나지 않는 반복이며(관측: 원본이 멈추지 않아 차분 실행 중 강제 종료), 후보는 `usize` 를 넘으면 `MemoryError` 로 끝낸다 — 동치가 아니라 의도한 종료 보장이다. (4) 타임아웃 폴링은 `time.monotonic` 대신 `Instant` 를 쓰며 지연 상한은 같다. (5) 터미널 줄 버퍼링은 `isatty(1)` 일 때 `print` 마다 flush 하는 모델이다(`sys.stdout.line_buffering` 의 관측 동작). (6) 도움말 폭은 COLUMNS 1..400 캡처에서 상수 구간까지(최상위 67 · prepare 191 · slice 231 · synthetic-sidecar 111 이상 동일). (7) Linux 전용 경로 모델(`os.fsencode` = UTF-8 surrogateescape). (8) 전체 1MiB 해시는 하드웨어(SHA-2 확장) 한계라 Python `hashlib` 과 거의 같은 속도다(아래 성능).

### 검사 (후보 · 이 작업 폴더에서 offline · 외부 target 디렉터리)

| 검사 | 결과 |
|---|---|
| `cargo fmt -- --check` · `cargo clippy --all-targets -- -D warnings` (lint 억제 없음) | PASS |
| `cargo build --release --bins` · `cargo test --release --no-fail-fast` | PASS (75 suite · 270 테스트 · 실패 0; R1~R9 suite 포함 영향 확인) |
| 고정 원본 순수 API 152 (`target_rows` 120 · `data_arg` 18 · `column_mapper_arg` 14; 값·예외 클래스·메시지·인자 불변) | PASS 152/152 (`dataset_pure_cases`) + 같은 152를 실제 `dataset_prep_api` 바이너리로 보낸 전송 검사 |
| 고정 원본 파일 IO 19 (`_load_spec` · `_dir_digest`: 유효·null 종류·키 없음·null/list/깨진 JSON·잘못된 UTF-8·BOM·부재·디렉터리·심볼릭 링크·빈 폴더·1MiB·끊긴 링크·유니코드 정렬·비-UTF-8 이름; 전/후 트리 전체) | PASS 19/19 (`dataset_io_cases`) |
| 고정 원본 CLI 83 (도움말·사용법 오류·`--self-test`·synthetic·slice·prepare; 종료·stdout·stderr(추적 투영)·전체 트리·가짜 docker 호출 기록) + 프로세스 출력 순서 2 | PASS 83/83 + 2/2 (`dataset_cli_cases`; PATH 의 실행 스크립트 고정물은 명령·부수효과 시험일 뿐 컨테이너를 만들지 않는다. 고정물이 자신의 실제 부모 PID·uid:gid 를 정규화하므로 하드코딩 값 없음) |
| PRNG 기준열 285 (CPython `random.Random(str).randrange` 의 40 추출 × 시드 15 × 경계 19: 비-ASCII·선행 NUL·300자·`seed:index`·경계 1~2^64−1) | PASS (`dataset_random_vectors`) |
| 터미널 출력(PTY): 배너가 자식 출력보다 먼저 | PASS (`dataset_terminal_cases`; 원본도 PTY 에서 같은 순서임을 직접 관측) |
| 소스 자체검사 T1~T7 를 실제 바이너리로 | stdout 이 원본 재실행·고정 원본과 바이트 동일, 종료 0 (`--self-test`) |
| 전송·측정기 검사 | `dataset_api_transport` 2 · `dataset_bench_harness` 3 · 측정기 단위 3 |
| **원본 대조 차분(자체 증거, 기준 변경 아님)** — 원본 `dataset_prep.py` 를 자식 프로세스로 실행해 종료·stdout·stderr(추적 투영)·전체 트리를 비교 | 최종 실행: slice+synthetic 600 · prepare 220(가짜 docker 9모드·캐시 적중/불일치/깨짐·타임아웃 0/−1/1/2/5) · 비-UTF-8 인자+COLUMNS 변이+argv 400; 개발 중 실행: 150 · 120 · 160 · 300 · 300(성공 70) · 220 — 전부 불일치 0 |
| 동적 타입 차분 — `cmd_synthetic_sidecar`/`cmd_slice` 를 원본 `argparse.Namespace`(인-프로세스)와 `dataset_prep_api` 에 같은 속성으로 호출 | 2000 + 500: 불일치 0 |
| 순수 함수 무작위 차분(`target_rows`·`data_arg`·`column_mapper_arg`, 임의 JSON 타입·NaN·큰 정수·문자열/딕셔너리 행) | 8000 × 7 시드 + 큰 입력 60: 불일치 0 |

### 실패한 시도·교정 (원시 로그는 비배포 작업 로그에 보존)

1. **순수 차분 첫 실행 52/4000 불일치** — `input_len` 이 NaN 일 때 전송이 디코더의 공유 NaN 객체(`PyValue::Shared`)로 넘기는데 `target_rows` 가 뺄셈 전에 껍질을 벗기지 않아 `TypeError`(원본은 `NaN > 8` 거짓 → 정상 값). `target_rows` 와 `int()` 호출(Namespace 속성·시드)에서 껍질을 벗기도록 교정, 재실행 0. 첫 실행의 앞 8건 발췌만 보존(전체 출력은 파일로 저장하지 못했다).
2. **차분 생성기의 거대 `--count`** — 원본이 `10000000000000000000000000` 개를 무작위 분기에서 끝없이 추출해 멈추지 않았다(원본 프로세스 2개를 강제 종료하고 값을 생성기에서 뺐다). 한계 (3).
3. clippy 첫 실행 실패 2건(`vec_init_then_push`·`useless_format`)은 코드로 고쳤다(`#[allow]` 로 가리지 않음). 한때 둔 `too_many_arguments` 억제도 `SliceRun` 구조체로 풀어 제거했다. 검사 도우미 `dataset_common.rs` 의 `#![allow(dead_code)]` 는 기존 도우미(`common.rs`·`reporting_common.rs`)와 같은 관례다 — 서로 다른 검사 크레이트가 한 도우미의 일부만 쓰기 때문이다.
4. 슬라이스 차분의 첫 생성기는 성공 경로가 150건 중 1건뿐이라 70% 를 깨끗한 입력으로 편향해 다시 돌렸다(성공 70/300). `pkill -f` 가 자기 셸을 죽여 한 번 실행이 끊겼다(재실행).
5. 성능 첫 측정에서 작은 `target_rows` 행 15개가 Python 보다 느렸다(복사 과다). 행을 빌려 쓰고 정수 키를 작은 값으로 두어 최악 0.94배(잡음 수준)로 줄였다.

### 성능 (자체 구현 증거, root 최종 아님 · 같은 전체 값/반환/해제 경계)

고정 순수 108개 × 200 × 11: Rust 7.65 ms / Python 19.25 ms = **2.51×**(행별 중앙 2.64×, 최악 0.94×). 실제 작은 IO 7개: 0.066 ms / 0.144 ms = **2.2×**. 1 MiB 전체 해시 1개: 0.45–0.57 ms / 0.47 ms ≈ **1.0×**(SHA-256 하드웨어 한계 — Python `hashlib` 해시만 0.42 ms; 스트리밍 읽기로 바꿨으나 개선이 없었다). 실제 CLI(프로세스 시작 포함, 중앙 11회): slice 1.6 ms / 22–36 ms(13–22×), synthetic 1.3 / 36.6 ms, prepare 캐시 적중 1.7 / 37.2 ms, 큰 slice 3.4 / 22.6 ms. 캐시·입력 축소·측정 경계 이동 없음. 이 수치는 현재 로컬 경계의 결과이며 LLM·서빙·통계적 일반화 성과가 아니다.

### 소스 → 시험 대응

`target_rows`·`data_arg`·`column_mapper_arg` → `dataset_pure_cases`·`dataset_api_transport` · `_load_spec`·`_dir_digest` → `dataset_io_cases` · `cmd_*`·`main`·argparse·`_die` → `dataset_cli_cases` · `dataset_random` → `dataset_random_vectors` · 줄 읽기/터미널 → `dataset_cli_cases`(깨진 UTF-8·chunk 사례)·`dataset_terminal_cases` · 측정기 → `dataset_bench_harness` · `_self_test` → `--self-test` 바이트 대조.

### R10 인수 정정 (종결 호출 · 소스 불변)

앞 절의 증거 범위를 정확히 하기 위한 기록이며 소스·검사·고정물은 바꾸지 않았다.

1. **로그와 보유 소스의 대응.** 마지막 소스 편집은 `target_rows` 의 빌림·작은 정수 키 최적화(성능 교정)이고, 그 뒤에 `fmt -- --check` · `clippy --all-targets -D warnings` · release 빌드 · `cargo test --release --no-fail-fast`(75 suite · 270 테스트 · 실패 0)와 `--self-test` 를 다시 돌렸다 — 이 최종 로그들은 보유 소스와 일치한다. 반면 **CLI/명령 수준 차분 최종 실행(slice+synthetic 600 · prepare 220 · raw+argv 400 · cmd 2000 · 순수 4×8000)은 그 최적화 이전 바이너리**로 돌았다. 최적화 이후에는 순수 무작위 차분 3×8000 과 큰 입력 60건(분포/해시 경로)을 다시 돌려 불일치 0 이었으나 **그 출력은 파일로 저장하지 않았다**(대화 기록에만 있다). 최적화 이후 CLI 차분은 다시 돌리지 않았다: `slice` 경로의 영향은 최종 `testfinal` 의 고정 원본 CLI 83·API 152·프로세스 2·PTY 1 로만 덮인다(소스 영향 메모).
2. **끝나지 않은/중단된 실행의 분류(PASS 아님).** `diff_first` 로그는 0바이트다 — 첫 차분 실행은 **원본 프로세스가 끝나지 않아**(`--count 10000000000000000000000000` 무작위 분기의 끝없는 반복) 강제 종료했고 결과 없음이다(원본 멈춤으로 분류, 후보 불일치로 분류하지 않음). 같은 원인으로 백그라운드에서 멈춘 `diff_slice2` 첫 실행의 출력은 같은 이름으로 재실행하면서 덮어써 **보존하지 못했다**(분실). 생성기에서 해당 값을 뺀 재실행만 저장되어 있다. 비결정/불안정(flaky)으로 분류된 사례는 없다(타임아웃 시나리오는 고정물이 3초 잠든 뒤 1~2초 한도에서 죽는 구조라 결정적으로 관측됨).
3. **진짜 후보 결함이 나온 실행.** 순수 차분 첫 실행 52/4000 불일치(NaN `input_len`)는 후보 결함이었고 고쳤다. 저장된 것은 앞 8건 발췌(`diff_pure_first_failure_excerpt.txt`)뿐이다.
4. **저장된 로그와 저장되지 않은 결과.** 저장: `final_slice` `final_prepare` `final_raw` `final_cmd_11/12` `final_pure_31~34`, 개발 중 `diff_slice1/2/3` `diff_prepare1/2` `diff_raw1`, `fmtcheck` `clippyfinal` `buildfinal` `testfinal` `testall`(첫 전체 시험, 73 suite 265 테스트), `selftest_native.*`, `time_cli`. 대화 기록에만 있음: 초기 `diff_cmd` 500건, 최적화 이후 순수 3×8000·큰 입력 60, 성능 비교(순수 108 · 작은 IO 7 · 1MiB · 행별) 수치. `time_cli` 의 CLI 시간은 최적화 이전 바이너리 값이다.
5. **공유 런타임 영향(재확인).** `pyfs.rs` 는 `read_in_blocks` 하나만 더했다(가산). `argv.rs` 는 `parse_with` 를 `parse_known_with` 위로 재구성했다(동작 보존; R1~R9 의 모든 argv 소비 suite 가 `testfinal` 에서 통과). 그 밖에 `pyvalue.rs`(`Layout::Tight`) · `cli.rs`(`on_large_stack` 공개, `flush_if_terminal`) · `lib.rs` · `Cargo.toml`(bin 3 · `[[test]]` 7 · `rustix` `process` 기능; lock 불변).
6. **미해결 의미 한계(변경 없음).** 앞 절 한계 (1)~(8) 그대로다. 특히 (3) 거대 `--count`(원본은 끝나지 않고 후보는 `MemoryError`)는 동치가 아닌 의도한 종료 보장이고, 1MiB 전체 해시는 하드웨어 한계로 Python 과 거의 같으며, 실제 Docker/GuideLLM/컨테이너는 증명 범위 밖이다. 독립 후보 PASS·여섯 프로파일·실제 Long E2E·정본 적용·신규 clone·root 성능은 모두 NOT_RUN 이다.

### R10 회귀 등록 — 비유한 `input_len` · 계수 해시 경로 (검사·자료만 추가, 소스 불변)

앞 절 3번의 교정된 결함(전송 디코더의 공유 NaN 이 껍질을 벗기지 않은 채 뺄셈에 들어간 `TypeError`)과 개수 8 초과의 해시 조회 경로는 넓은 무작위 차분에서만 덮였고, 최적화 이후 성공 출력은 저장되지 않았다. 이를 막는 명시 회귀를 `checks/dataset_api_transport.rs` 의
`non_finite_input_lengths_and_many_distinct_counts_answer_through_the_wire_like_the_original` 로 등록했다(기존 `[[test]] dataset_api_transport`, Cargo 변경 없음). 자료 `checks/data/dataset_prep/numeric_wire_cases.jsonl` 는 원본이 먼저 낸 9건(`{id, operation, arguments, expected}`: 빈/정수/bool/비정수 행 × NaN·±Infinity, 큰 정수 − NaN 의 `OverflowError`, 일반/해시 모드의 8 초과 계수·bool)이며 **실제 `dataset_prep_api evaluate-jsonl` 자식 프로세스**에 `id`·`operation`·`arguments` 만(NaN/Infinity 토큰 유지, `expected` 는 보내지 않음) 보내 전송 디코더를 거친 전체 값·예외 종류·메시지를 비교한다 — 도우미가 읽은 평범한 `Float(NaN)` 에 `evaluate_operation` 을 부르는 방식은 이 결함에 닿지 못한다. 변이 확인: 임시 사본에서 `subtract(&target, input_len.peel())` 의 `.peel()` 을 빼면 이 검사가 `int-nan` 에서 실패한다(로그 `numreg_mutation_unpeeled`; 사본은 지웠고 후보 소스는 건드리지 않았다). 원본 대조 기록(`original-cases`·결과)은 root 가 보관한다.


## R10 정본 통합 판정

데이터셋 전처리·캐시·행 선택·sidecar와 실제 run_bench의 세 CLI/두 helper 호출을 Rust로 연결했다.
정본 build12자체검사·fmt·release271(75suites)+별도FD22·Clippy·외부171API/85CLI·실제caller2가 PASS다.
독립2 actualCLI는 원본재실행 없이 hardlink/Unicode 실패와 stdoutPTY/cleanup/partial 파일을 보존했고,
원본먼저 추가9 숫자 경계를 strict 타입까지 확인했다. 이9개는 실제 API wire decoder를 거치는 등록 회귀이며
SharedNaN peel 삭제 변이에서 실패했다. 격리 Python transport·missingbinary7, Git-index537(Rust489+의존48)
소스만 새 공백 경로로 옮긴 offline build12와 등록dataset7targets/package_paths도 PASS다.

첫 배포130은128PASS/2FAIL이었다. R9의 오류 안내가 미정의 SDIR를 사용한 실제 회귀를 정의된 REPO 경로로
고쳤고 L4의 기존 요구를 그대로 재실행했다. 해시 알고리즘 시험의 scalar 기대값은 임시 read-only 입력의
정답이며 추적 blob 무결성 metadata와 구분했다. 다른metadata는 그대로검사한다. 새 분류기의 duplicate-key
누락D1은 독립2 FAIL로 확인해중복키원문검사/numeric토큰보존으로고쳤고같은2개PASS다. 실패2명령을
영향 재검증하고 유효128개를 재사용해 현재130항목을충족했다. 두번째전체배포를실행했다고하지않는다.
R9의후검사안내변경뒤125항목재사용판정은L4부분에유효하지않았음을외부기록에정정했다.

사전11교차round의전체반환/해제: pure108×200 Python771.091806ns/Rust358.389676ns=2.151546배,
실제smallIO7 20589.714286ns/19263.857143ns=1.068826배, 1MiB파일전체digest는587437/461406ns=1.273146배다.
실제 Bash CLI6 round평균중앙값28.174198/4.635046ms=6.078516배이며개별6개는
5.073115/8.367079/7.872064/7.109388/4.739530/3.666461배다. 모든값·출력·종료·파일을대조했다.
전처리캐시hit는실제전체해시와캐시읽기이며fake Docker 전처리속도를성과로세지않는다.
입력/측정경계/캐시를유리하게바꾸지않았고actual Docker/GuideLLM·LLM·서빙·통계적일반화의성과가아니다.

Native첫SDK120턴예산은121관측턴error_max_turns였다. 같은세션문서closeout과test-only회귀등록으로
총132턴/reported2133.080초/wrapper2144.320초·denials0을기록했다. 일부과거fuzz원시가덮인한계는보존하고
파일에없던3×8000/large60의실제tool출력은외부native전사에서회수했다. 원본의완료를관측하지못한
주소공간초과count는Rust usize변환에서MemoryError이며성공원본동등성/모든큰입력의종료보장을뜻하지않는다.
전체 S1~S7·하네스·6프로필·실제single4/multi2LongE2E는계속미완료다.

## R11 측정 도구 핀 해소(`resolve_bench_tool`) 이행 — 후보 기록 (정본 적용 전)

상태: **COMPLETE — R11 범위(원본 `resolve_bench_tool.py` 254줄 + `bench_tool_pin.json` 한 건)의 후보와 자체검사만.**
정본 적용·정책/등록부 연결·공식 성능·독립 검증은 이 기록의 범위 밖이며 아래 "미연결과 잔여"에 있다.

### 산출물

| 경로 | 내용 |
|---|---|
| `src/bench_tool_pin.rs` | 라이브러리: `load_pin`, `accepted_digests`, `classify`, `inspect`(`_inspect`), `default_pin_path`, 상수 |
| `src/bench_tool_cli.rs`, `src/bench_tool_selftest.rs` | argparse `main`(원본 사용법·오류·종료 코드), P1~P9 자체검사 |
| `src/bench_tool_api.rs` | Rust 전용 전송 `evaluate-jsonl` / `constants` / `native-help` |
| `src/help_layouts_resolve_bench_tool.rs`, `src/argv.rs`(`BENCH_TOOL`) | 터미널 폭 1..700의 사용법·도움말 배치(원본 파서 출력 캡처), 파서 표 |
| `src/bin/resolve_bench_tool.rs` · `_api.rs` · `_bench.rs` | 원본 CLI · 전송 · 일반 전체호출 시간측정기 |
| `checks/bench_tool_*.rs`, `checks/data/resolve_bench_tool/` | 자체검사 7개 대상(아래)과 고정 기준 데이터 |
| 기존 R10 파일의 가시성만 변경 | `dataset_commands::{exec_argument, spawn_failure}`, `dataset_prep_cli::{StreamsConsole, exit_status}` 를 `pub(crate)` 로(동작 불변, R1~R10 검사 전부 재통과) |

의존성 추가 없음(`Cargo.lock` 불변).

### 원본 → Rust 대응

| 원본 | Rust | 비고 |
|---|---|---|
| `PinError(ValueError)`, `load_pin(path, tool, version=None)` | `bench_tool_pin::load_pin` → `PyDict` | 실패는 `Failure{class:"PinError"}`; 행은 키·순서 보존 + `version` 마지막(이미 있으면 제자리) |
| `accepted_digests(entry)` | `accepted_digests` → `AcceptedDigests`(문자열 집합 + "문자열 아닌 참 index_digest" 표시) | 내부 집합. **외부 와이어 연산이 아니다**(집합은 JSON 형태가 없다). 외부 수용은 `classify` + `load_pin` |
| `classify(entry, repo_digests)` | `classify` → `Classification{resolution, observed_digest, matches_record}` | 전송에서는 `[rc, observed, matches]` 배열. 후보 선택은 원본의 `sorted` 최소 일치 |
| `_inspect(image_ref)` | `inspect` → `Option<Vec<PyValue>>` | `docker image inspect IMAGE --format {{json .RepoDigests}}` 한 가지, timeout 120, 두 스트림 포획(strict UTF-8 + 보편 개행). **pull 하지 않는다** |
| `_self_test()` P1~P9 | `bench_tool_selftest::self_test` | 출력 바이트가 원본과 같다(고정 골든) |
| `main(argv)` | `bench_tool_cli::main_with` / `run` | 종료 0 해소 · 2 인자/기록 오류 · 3 이미지 부재 · 1 미처리 예외(트레이스백 마커 + 마지막 줄) |
| `RC_OK/RC_ARGS/RC_ABSENT`, `DEFAULT_TOOL`, `PIN_RELATIVE` | `bench_tool_pin` 상수, `default_pin_path()` | 아래 "기본 핀" |

타입과 소유권: 판정(어느 행·어느 digest·어떤 상태)은 `Resolution`/`Classification`/`AcceptedDigests` 로 닫고, 핀 파일이나 호출자에게서 온
값은 원본이 어떤 JSON 모양이든 받아 파이썬 예외로 실패하므로 `PyValue` 를 유지했다(`.get` 의 AttributeError, 해시 불가 TypeError,
`dict(row)` 의 ValueError/TypeError 등). 파이썬 판정 로직을 런타임 브리지로 옮기거나 원본 파이썬을 호출하지 않는다.

### 보존한 특이 동작

- 빈/거짓 `version` 은 `default_version` 으로 대체, 비어 있지 않은 잘못된 형은 원본 오류(`TypeError: unhashable type`, `PinError ... 버전 'x' 이 없다`).
- `dict(row)`: 객체가 아닌 행(쌍 목록·문자열·스칼라·None)도 `dict(...)` 의 의미와 메시지로 처리, 문자열 아닌 키 허용.
- digest 불일치는 **막지 않는다**(stderr NOTE만, 종료 0). 이미지 부재만 종료 3, 자동 pull 없음. `RepoDigests` 가 리스트가 아니면 부재.
- 플랫폼으로 좁히지 않는다(`--platform` 은 출력 주석).
- 비정상 종료 코드의 `docker` 도 두 스트림을 먼저 해독한다 — 해독 불가 바이트는 종료 코드와 무관하게 `UnicodeDecodeError` 트레이스백(검사 포함).
- 반복 옵션은 마지막 값, 약어 허용, 원본 도움말·오류 문구와 터미널 폭 의존 배치(700폭 전수 대조).

### 기본 핀(owner context)

원본 `PIN_RELATIVE` 는 스크립트 옆 `../bench_tool_pin.json` 이다. Rust 는 환경변수 `BENCH_TOOL_PIN_PATH`(빈 값 = 미설정)를 먼저 읽고,
없으면 빌드한 크레이트의 부모 폴더의 `bench_tool_pin.json`(노드에서 크레이트를 제자리 빌드한 맥락)을 쓴다. 얇은 런처가
`${BENCH_TOOL_PIN_PATH:-$SDIR/../bench_tool_pin.json}` 으로 내보낸다. 사용자 `--pin PATH` 는 그대로 이긴다. 소스에 개발자 경로는 없다.

### 실행 근거 (모두 후보 폴더 기준, 외부 `CARGO_TARGET_DIR`, 오프라인·`--locked`, 관리 도구체인)

| 항목 | 명령 | 결과 |
|---|---|---|
| fmt | `cargo fmt -- --check` | PASS (exit 0) |
| clippy | `cargo clippy --release --all-targets -- -D warnings` | PASS (exit 0) |
| build | `cargo build --release` | PASS |
| 전체 테스트 | `cargo test --release --no-fail-fast` (환경: 기존 R1~R10 검사용 변수 + `BENCH_TOOL_PIN_PATH`=원본 831 기록) | PASS — 통과 291, 실패 0, 무시 0 (기존 R1~R10 검사 포함 · 필터된 대상 0은 세지 않음) |
| 고정 순수 기준 | `bench_tool_pure_cases` | PASS — `classify` 96건(값 53 · 예외 43, 인자 불변 포함) |
| 고정 파일 기준 | `bench_tool_io_cases` | PASS — `load_pin` 25건(실제 파일, 전후 트리 전체 비교) |
| 고정 CLI 기준 | `bench_tool_cli_cases` | PASS — 45건(상태·stdout·stderr·전후 트리/엔진 호출 기록) + 자체검사 골든 + docker 부재 + 부재 이미지 inspect 1회·pull 0 + 해독 불가 스트림 3종 = 5개 테스트 |
| 전송 | `bench_tool_api_transport` | PASS — 순수 96·파일 25가 바이너리 `evaluate-jsonl` 로 동일, 답 모양 6종, 네이티브 명령 |
| 행 모양 | `bench_tool_row_shapes` | PASS — `dict(row)` 12 모양, 디지스트 집합 |
| 폭 | `bench_tool_width_cases` | PASS — 배치 변화 지점 + 80·700 (65폭) / 개발 시 700폭 전수 대조 0 차이 |
| 측정기 | `bench_tool_bench_harness` | PASS — 3 (기대값 불일치·실패·진단·`expected_json` 오류는 시간 JSON 없이 종료 1) |
| 런처(임시 노드 트리) | 얇은 런처로 self-test·기본 핀·`--pin` 우선·바이너리 부재 | PASS — 부재 시 종료 2, 자동 빌드 없음, 엔진 호출은 inspect 만 |

고정 기준 데이터: 원본에서만 수집된 참조(순수 96 · 파일 25 · CLI 45)를 `checks/data/resolve_bench_tool/` 로 옮겼다. 값·메시지·종료 코드는
그대로이고, 트레이스백은 확립된 투영(마커 앞 텍스트 + 마지막 줄)만 쓰며 개발자 argv 메타데이터는 뺐다. 엔진은 `PATH` 앞의 `.fixture-bin/docker`
스크립트(실행 권한은 검사가 복원, 환경변수로만 응답, 호출 전체 기록)이며 실제 docker·이미지 질의·네트워크는 쓰지 않았다.

작업자 자체 프로파일링(공식 성능 아님 · 공식 측정은 루트 소관): 순수 53×200 11라운드 라운드당 약 1.76 ms(호출당 약 0.17 µs),
실제 `load_pin` 6건 11라운드 라운드당 약 21 µs. 파이썬과 비교하거나 속도 우위를 주장하지 않는다.

### 알려진 한계 / 확인하지 못한 것

- **NOT_RUN** `inspect` 의 120초 `TimeoutExpired` 경로(종료 코드·메시지는 구현했으나 120초 지연 픽스처로 실행하지 않았다).
- `text=True` 는 로케일 인코딩을 쓴다. 고정 기준은 UTF-8 환경이며 Rust 는 항상 strict UTF-8 이다(다른 로케일은 비대응).
- `load_pin` 의 경로 인자가 정수/불리언(파일 기술자)이면 `NotImplementedError` 로 거절한다(원본은 기술자를 연다 — 모사하지 않음). 그 밖의 비문자열은 `TypeError`.
- 문자열 아닌 키의 동치는 `1`/`1.0`/`True` 와 NaN(매번 별개)까지만 구현했다. JSON 문서에서는 키가 항상 문자열이라 파일 경로로는 닿지 않고 쌍 목록 행에서만 닿는다.
- 터미널 폭 배치는 원본 파서를 개발 시 프로세스 안에서 가져와 폭 1..700 의 출력을 캡처한 데이터다(이행 동작의 기준 실행이 아니라 도움말 배치 데이터 생성). 데이터 생성 외에 원본을 다시 실행하지 않았다.
- `default_pin_path()` 의 크레이트-부모 대체 경로는 빌드 시 `CARGO_MANIFEST_DIR` 에 의존한다(소스 상수 아님). 런처를 쓰면 환경변수가 우선이다.

### 미연결과 잔여 (루트 소관, 이 후보는 연결하지 않았다)

- 정본 적용·`resolve_bench_tool.py` 처분, 얇은 런처 2종 설치, `build_rust.sh` 의 이진 목록, 공식 성능·신규/독립 검증.
- 소비자: `run_bench.sh` 의 CLI 두 호출(`--json` 해소, 실패 사유 재호출), `verify_distribution.py` 의 `--self-test` 호출, 정책 C10(구 파이썬 텍스트 스캔 → 실제 Rust CLI의 pull 없음·부재 3·성공 0·드리프트 provenance 동작 검사),
  `base_skill_edges.json`/`claim_bindings.json`/`registry.yaml`/`claim_predicates.py`/`antipattern_scan.py` 의 `resolve_bench_tool.py` 식별자·경로.

## R11 교정 — `inspect` 캡처의 단일 총 기한 (후보 · 정본 적용 전)

**실패한 증인.** 직접 자식이 유효한 응답을 쓰고 포크한 자손에게 상속 파이프를 쥐여 둔 채 먼저 종료 0 으로 끝나는 엔진 픽스처에서,
원본은 `subprocess.TimeoutExpired`(120초)로 종료 1·빈 stdout 이고, 최초 후보 릴리스는 124.01초 만에 종료 0·JSON 성공을 돌려줬다(FAIL, 보존).
원인: `run_captured` 가 자식 종료를 보면 유일한 기한 루프를 빠져나와 두 판독 스레드를 **기한 없이** `join` 했다 — 파이프가 열려 있는 한
120초 제한이 적용되지 않았다.

**구현 변경(`src/bench_tool_pin.rs`, 캡처 전송부만).** 판독 스레드를 없애고 한 스레드가 두 파이프를 비차단으로 `poll` 한다.
하나의 총 기한이 (1) 두 스트림의 EOF 도달 (2) 자식 종료 확인 둘 다를 덮는다. 자식이 먼저 끝나도 파이프가 열려 있으면 기한까지 기다린 뒤
`TimeoutExpired`(`Command '[...]' timed out after 120 seconds`)로 끝낸다. 기한이 지나면 **직접 자식만** kill+wait 로 거둔다(자손·프로세스 그룹은 건드리지 않는다).
오류 경로도 같은 정리(kill+wait)를 하고 파이프는 함수 반환 시 닫히므로 호출보다 오래 사는 스레드가 없다. 보존: 정확한 인자
`docker image inspect IMAGE --format {{json .RepoDigests}}`, stdin 상속, 출력 무절단 수집, 두 스트림 strict UTF-8 해독 뒤 종료 코드 판정, 보편 개행,
종료/시그널(음수) 상태, 예외 클래스·메시지. 기한은 비공개 경계 `run_captured_within(command, shown, limit)` 로 주입 가능(운영은 `INSPECT_TIMEOUT` = 120초).
`Cargo.toml`: 기존 의존성 `rustix` 에 `event`, `time` 기능만 추가(새 크레이트 없음, `Cargo.lock` 불변, `--locked` 통과).

**등록된 회귀(`bench_tool_pin::capture_tests`, 같은 운영 구현을 실제 `/bin/sh` 자식으로 호출, 자식·자손 수명 유한).**
(1) 조기 종료 직접 자식 + 파이프 쥔 자손 → 0.3초 한도에서 `TimeoutExpired`, 경과 0.3~2.5초, 자식 회수(`/proc` 부재) (2) 응답 없는 살아 있는 자식 → kill·회수
(3) 스트림을 닫고 계속 도는 자식 → 기한 (4) 양성 대조: 파이프 버퍼보다 큰 stdout 30만 바이트와 stderr 보존·종료 3, 시그널 −9, 파이프를 쥐지 않는 자손은 기다리지 않음
(5) 운영 한도 120초와 메시지의 초 표기. 타이머를 따로 복제한 시험이 아니다.

**실제 명령과 결과**(오프라인·`--locked`·외부 target·관리 도구체인):
`cargo fmt -- --check` PASS · `cargo build --release` PASS · `cargo clippy --release --all-targets -- -D warnings` PASS ·
`cargo test --release --lib --test bench_tool_{pure_cases,io_cases,cli_cases,api_transport,row_shapes,width_cases,bench_harness}` PASS
(lib 100 통과 — 신규 5 포함, 고정 기준 96/25/45 및 전송·폭·측정기 전부 재통과, 실패 0).
같은 증인에 교정된 릴리스: **종료 1 · 빈 stdout · 120.04초 · 마지막 줄이 원본 raw 와 바이트 동일**
(`subprocess.TimeoutExpired: Command '['docker', 'image', 'inspect', 'fixture/image:0.7.3', '--format', '{{json .RepoDigests}}']' timed out after 120 seconds`), 홀더는 헬퍼가 정리, 실제 Docker·네트워크 0.
원본은 다시 실행하지 않았고 고정 기대값은 수정하지 않았다. 전체 스위트는 루트가 정본 적용 후 돌린다(이번 교정에서 돌리지 않음).

**한계.** 이 교정은 `inspect` 캡처만 바꿨다 — 독립 해독 증인은 캡처 경로가 바뀌어 루트가 필요한 델타만 재요청한다. 120초 기본값 자체를 단위 시험에서 소모하지는
않았다(주입 한도로 같은 구현을 검증, 실제 120초는 위 증인 한 번). `poll`/`read` 의 비차단 EAGAIN 처리는 리눅스에서만 검증했다. 성능 측정은 하지 않았다.

## R11 교정 2 — 스트림 드레인의 구조적 기한·공정성 (후보 · 정본 적용 전)

**출처 구분.** 이것은 관측된 실행 불일치가 아니라 **소스 리뷰 지적**이다. 앞선 120초 파이프 보유 증인(종료 1·빈 stdout·`TimeoutExpired`·120.04초, 원본 raw 와 일치)은
실제 실행 FAIL 이 교정되어 PASS 한 기록이며, 이번 변경은 그 조건을 바꾸지 않으므로 **재실행하지 않고 그 증인을 그대로 재사용**한다. 원본도 다시 실행하지 않았다.

**지적.** `Pipe::read_available` 이 EAGAIN/EOF 까지 기한 확인 없이, 호출당 바이트 제한 없이 반복 → 계속 보충되는 stdout 이 `capture_until` 의 유일한 기한 확인으로의
복귀를 막고 stderr 도 굶길 수 있다(원본 `communicate` 는 기한 확인 사이에 제한된 읽기를 한다).

**변경(`src/bench_tool_pin.rs`).** `read_available` → `read_once`: 호출당 **읽기 한 번**(`READ_BLOCK` = 65536 바이트 이하). EOF 는 파이프를 닫고, EAGAIN·EINTR 은 바깥 루프의
기한 확인으로 돌아간다. 바깥 루프는 매 반복 기한 확인 → `poll` → stdout 한 번 → stderr 한 번 읽는다(공정). 읽은 바이트는 버리지 않고 반복으로 누적하므로 완전 수집은 유지된다(잘림·캐시·대체 경로 없음).

**등록된 회귀(`bench_tool_pin::capture_tests`, 신규 2 + 기존 5 유지).**
(1) `a_continuously_replenished_stream_does_not_defeat_the_deadline` — 직접 자식이 최대 64 MB 를 계속 쓰는 실제 생산자, 같은 운영 구현 `run_captured_within` 을 0.1초 한도로 호출 → `TimeoutExpired`·1.5초 미만·자식 회수(메모리는 생산 한도 64 MB 이하, 생산자는 자식 자신이라 기한에 죽음 · 고아 없음)
(2) `one_read_is_bounded_and_keeps_every_byte` — 호출 한 번이 `READ_BLOCK` 을 넘지 않고 1,000,000 바이트가 반복으로 빠짐없이 모인다(기존 구조에서는 이 경계가 존재하지 않음).
기존 양성 대조(30만 바이트 완전 stdout/stderr·종료 3·시그널 −9·파이프 없는 자손)와 세 가지 기한 시나리오는 그대로이며 통과한다.

**명령과 결과**(오프라인·`--locked`·외부 target): `cargo fmt -- --check` PASS · `cargo build --release` PASS · `cargo clippy --release --all-targets -- -D warnings` PASS ·
`cargo test --release --lib --test bench_tool_{pure_cases,io_cases,cli_cases,api_transport,row_shapes,width_cases,bench_harness}` PASS (lib 102 통과 — 캡처 7 포함; 고정 96/25/45·전송·폭·측정기 재통과, 실패 0).
전체 스위트·121/45 재생·호출자 4·독립 해독 델타는 루트 소관이며 돌리지 않았다.

**한계.** 흐름 시나리오(1)의 생산자는 이 기계에서 판독기보다 느려, 구(舊) 무제한 드레인에서도 통과했을 수 있다 — 구조적 경계를 확정하는 것은 (2)이고 (1)은 기한·정리 동작의 실행 확인이다(구 구현에 대한 돌연변이 시험은 하지 않았다).
`READ_BLOCK` 은 한 번에 읽는 상한이지 출력 상한이 아니다(출력은 기한 안에 생산된 만큼 메모리에 쌓인다). 리눅스에서만 검증했다. 성능 측정은 하지 않았다.


## R11 정본 품질·성능 인수

로컬 도구 해소기의 실제 호출은 Bash/Rust로 연결했다. 정본40경로의 기존mode 변화0,
명시 build13자체검사·fmt·Cargo release298(85결과suite)+별도FD22·Clippy가 통과했다.
Root 고정121API/45CLI·실제 run_bench 해소 구간4·독립 최종decode2와 source/policy 검토가 통과했다.
전체 배포검사130/130도 한 번의 전체 실행에서 통과했다. Git-index561(Rust510+전달의존51)만
공백 포함 새 경로로 복사한 offline build13·관련7등록대상/라이브러리/패키지 경로 검사도 통과했다.
첫 정본/fresh Cargo는 검사 소유자 입력 BENCH_TOOL_PIN_PATH가 없어 NOT_RUN assertion으로 실패했다.
원시를 보존하고 실제 배포 pin 경로를 명시해 같은 필요한 검사를 완료했으며 제품 코드는 바꾸지 않았다.

실제 inherited-pipe 경계에서 원본은120초 TimeoutExpired/exit1/빈stdout, 최초 후보는124.01467초 성공이었다.
자식 종료 뒤 deadline 없는 reader join을 단일 스레드 nonblocking poll/소유FD 정리로 교정했다.
첫 수정의 실제120.04067초 실패는 원본의 예외와 일치했다. 추가 소스검토의 연속 drain 문제는
각stream당 최대65536bytes 한 번 읽고 deadline을 재확인하는 방식으로 교정했다.
이 최종 차이는 같은 조용한 inherited-pipe 조건에 영향을 주지 않아120초 관측을 조건부 재사용했고,
독립 source검토와 최종decode2 실제 대조로 확인했다. 최종bin의 새120초 측정이라고 하지 않는다.
등록7개 capture 회귀는 실제 프로세스/정리/전체출력/지속출력과 bounded-read 조건을 검사한다.
지속출력 검사 하나만으로 이전 drain 결함을 반드시 구별한다는 주장이나 mutation 증명은 하지 않는다.

C10의 예산/teardown/endpoint/serve 불변식은 유지하고, 설치 Rust CLI의 일치/부재3/드리프트0과
image-inspect-only 동작검증으로 이전 Python source-text 증거를 대체했다. registry/binding/간선과
Rust 제거 tripwire를 연결했고 전체 정책·배포 검사가 통과했다. 원본3 수용/변이4 거부와 별도 source검토는
각각의 근거이며 모든 가능한 회귀의 형식적 보장으로 확대하지 않는다.

사전11교차round: pure53×200은 Python571.483491ns/Rust335.587264ns=1.702936배,
실제load_pin6은7253.333333ns/4386.666667ns=1.653495배다.
실제 Bash CLI6의 round평균 중앙값은34.595252ms/16.618566ms=2.081723배다.
명시 engine 대역4는39.172149ms/22.390039ms=1.749535배, engine 없는2는24.784297ms/4.727691ms=5.242368배다.
개별 matching1.609571/drift1.530936/absence1.605611/version오류6.991523/selftest4.199842/platform1.489978배다.
모든 전체반환·출력·종료·파일을 대조했고 warm 검증과 timed 반환/해제를 포함했다. 대역 프로세스 비용도
포함했으며 실제Docker/image조회 속도·LLM/서빙·프로필·통계 일반화의 성과가 아니다.

Native는 같은 세션의75+10+5=90턴, reported1368.628초/wrapper1380.944초, 권한거부0이다.
관측 모델은claude-sonnet-5-5/high요청·실제effort미보고다. 정수/불린 pin경로(fd) API는 명시 미지원이며
생산 CLI 경로는 문자열이다. 텍스트 환경은 UTF-8에서 확인했고 모든 locale의 동등성을 주장하지 않는다.
기존 Python 원문과 후속 inline glue는 남아 있다. 연결 원장은16/128모듈이며 줄커버리지/Python0이 아니다.
전체 S1~S7·하네스·6프로필·실제single4/multi2 LongE2E는 계속 미완료다.


## R12 GuideLLM 스테이징 해소기(`resolve_guidellm`) 이행 — 후보 기록 (정본 적용 전)

**상태: COMPLETE — 이번 후보와 자체검사 범위에 한한다.** 성능·실제 호스트 통합(런처/`build_rust.sh`/Terraform)·정본 적용·독립 대조·전체 S1~S7·하네스·6프로필·실제 Long E2E 는
이 기록의 주장이 아니다(Root 소관·미완료). 원본 `resolve_guidellm.py`(219줄)와 `bench_tool_pin.json` 은 실행하지 않았다(동결 참조만 읽음).

### 산출물과 책임 (이름: 기존 `guidellm.rs` 는 결과 *파서*라 이 모듈은 `guidellm_release`)
| 경로 | 책임 |
|---|---|
| `src/guidellm_release.rs` | `version_from_tag`·`image_ref`·`resolution`·`fetch_latest`(전송 trait)·`merge_record`·요청 구성·실패 분류. 정규식 `^\d+(?:\.\d+){1,3}[-.a-zA-Z0-9]*$` 은 유니코드 숫자 `\d` 포함 선형 시간 판정기 |
| `src/guidellm_release_http.rs` | 운영 HTTPS 전송: `ureq =3.5.0`(rustls+ring · platform-verifier · gzip/charset/json/cookie 없음). 원문 바이트·본문 상한 없음·정체성 인코딩·`https_proxy`/`no_proxy`(urllib 규칙)·작업별 20초 |
| `src/json_bytes.rs` | `json.loads(bytes)`: `detect_encoding` + `surrogatepass`(utf-8·utf-8-sig·utf-16·utf-32 BOM/비BOM) + CPython 오류문 |
| `src/guidellm_release_cli.rs` · `_selftest.rs` | 원본 argparse 표면(약어 포함)·종료코드 0/2/3(+미처리 예외 1)·T1~T9 |
| `src/guidellm_release_api.rs` | `resolve_guidellm_api evaluate-jsonl/constants/native-help`; API 전용 주입 전송 `fetch_fixture`(`response_hex` 또는 `failure{type,message}`); 고정 없으면 `RuntimeError`(API 는 네트워크에 닿지 않는다) |
| `src/bin/resolve_guidellm{,_api,_bench}.rs` | 원본 CLI · API 전송 · 범용 전체 호출 계측기(행 파일·라운드·반복·id) |
| `src/help_layouts_resolve_guidellm.rs` | 폭 1..700 usage/help(argparse 3.12.3 출력, 폭 129 이상 상수) — 원본을 실행하지 않고 **같은 파서 정의를 재생**해 생성, 기본 폭에서 동결 원본 출력과 대조 |

공유 소스 변경(최소): `argv.rs`(RESOLVE_GUIDELLM 표 추가) · `pyvalue.rs`(`PyDict::get_mut`·`setdefault_key`·`json_key` crate 가시성) · `bench_tool_pin.rs`(`dict_key` crate 가시성) ·
`json_stream.rs`(문자열이 아닌 dict 키를 `json.dump` 처럼 `null`/`true`/숫자로 적는다 — 이전엔 repr 문자 그대로였던 잠재 결함) · `lib.rs` · `Cargo.toml/lock`.

### 보존한 외부 계약·특이 동작
선언 `--version` 은 검증하지 않음(`--version=` 빈 값은 최신 분기) · 최신은 실제 릴리즈 태그에서만(옛 버전 fallback 없음) · `published_utc` 는 상류 값(벽시계 없음) · `--record-path`(빈 문자열 포함)가 환경 `GUIDELLM_RECORD_PATH` 보다 우선,
환경이 없으면 크레이트 상위 `bench_tool_pin.json`(소스에 개발자 경로 리터럴 없음) · `--record` 는 기존 버전/미지 필드/digest/플랫폼을 보존하고 새 버전을 기본으로(키가 `null`/불린/숫자여도 `json.dump` 철자) ·
`open("w")` 절단 → 청크 단위 부분 dump → 실패 시 그 접두부만 남음(하드링크 같은 inode 가 따라옴) · 예외 순서(`setdefault` 연쇄 → `res["version"]` → 행 채움) ·
HTTP: 원본이 `ValueError` 로 잡는 `JSONDecodeError`/`UnicodeDecodeError` → 종료 2, `URLError`/`HTTPError`/`TimeoutError`/`OSError` → 종료 3 + 원문 UNRESOLVED 문구, 그 밖은 traceback 표지 + `Class: message` 종료 1 ·
요청: `GET https://api.github.com/repos/<repo>/releases/latest`, `Accept: application/vnd.github+json`, `User-Agent: adversarial-benchmark`(+`Accept-Encoding: identity`) · `http.client` 사전검증(제어문자 `InvalidURL`·비ASCII `UnicodeEncodeError` 는 연결 전)·`Request` 의 마지막 `#` 조각 분리.

### 명령·결과 (모두 managed toolchain pin 1.99.1 aarch64 · 후보 크레이트 · 외부 target(작업 영역의 `cargo-target`) · 로그는 작업 영역 `logs/`)
| 항목 | 명령 | 결과 |
|---|---|---|
| 의존성 fetch(기능 요청과 별도 계수) | `cargo info ureq@3.5.0`(인덱스 갱신+크레이트 1건) · `cargo fetch`(공개 crate 59건, 프로젝트 관리 CARGO_HOME 만) | PASS · `logs/cargo-info-ureq.log` `logs/cargo-fetch-01.log` · 이후 전부 `--offline --locked` |
| lock 변경 | ureq 선언 시 신규 crate(ring 단일 · aws-lc/openssl 링크 없음 · `cargo tree` Linux 트리 `logs/cargo-tree-01.log`) · dev-dep `rustls`(ring+std) 간선 1줄을 `cargo metadata --offline` 으로 기록(신규 crate 0) | PASS · 첫 `--locked` 실패(간선 미기록) 후 offline 갱신 · `logs/cargo-metadata-lock-edge-01.err` |
| fmt | `cargo fmt` 적용 후 `cargo fmt -- --check` | PASS (`fmt-apply-01/02`, `fmt-check-01`) |
| 빌드 | `cargo build --release` (3 bin 포함) | PASS (`build-final-01`) · sha256 `logs/release-binaries-sha256.txt` |
| Clippy | `cargo clippy --release --all-targets -- -D warnings` (lint 억제 없음) | PASS (`clippy-02`) |
| 전체 테스트 | `cargo test --release --no-fail-fast` | PASS exit 0 · 95 결과 블록 · 334 테스트 통과·0 실패 (`test-full-01`; 서식 적용 뒤 신규 9대상 재실행 PASS `test-05`) |
| 동결 TLS6 비공개 확인 | `check_tls_r12_private.py`(Root 비공개 helper 읽기 전용 import) | PASS 6/6: exit/stdout/stderr/전체 트리/GET·경로·Accept·User-Agent·Host (`logs/tls-private-01.log` · 작업 영역 `evidence-tls-01/results.json`) · 원격 요청 0 |
| 런처 초안 | 임시 노드 트리에서 selftest·선언·바이너리 없음(exit 2)·API constants | PASS · `drafts/scripts/` |

### 동결 기준 ↔ 등록 검사 (모두 `checks/`, `autotests=false`)
| 동결 기준 | 검사 | 수 |
|---|---|---|
| 1 pure API 48(값36·예외12, arguments_after) | `guidellm_release_pure_cases` · 이진 경유 `guidellm_release_api_transport` | 48 |
| 2 merge IO 19(값11·예외8, 전/후 전체 트리) | `guidellm_release_io_cases`(+하드링크 inode 추적) | 19 |
| 3 원본 실제 CLI 36(exit 0×20·1×11·2×5) | `guidellm_release_cli_cases`(+selftest golden·record-path 우선/빈값·약어·빈 `--version`→최신 분기를 닫힌 루프백 프록시로 확인) | 36 |
| 4 주입 fetch 30(값18·예외12) | `guidellm_release_fetch_cases`(요청 url/method/headers/timeout20 + 본문 종료 1회 프로브: 값·JSON 실패·디코드 실패·읽기 실패) · 이진 경유 `guidellm_release_api_transport` | 30 |
| 5 실제 TLS CLI 6 | 비공개: 위 TLS6 / 배포용: `guidellm_release_tls_cases`(실행 시점 임시 CA·rustls 서버·CONNECT 터널·child env 전용 신뢰: 정상·UTF-16 BOM·11MiB 본문·404·미신뢰 인증서=소거 불가 실패) | 6 / 4 |
| 계측 계약 | `guidellm_release_bench_harness`(전체 기대값 warm 1회·오류/진단 거부·네트워크 거부·merge 실파일) | 4 |
| 단위 | `json_bytes`(5)·`guidellm_release`(3)·`guidellm_release_http`(1)·계측기(3) | 12 |

### 실패·수정 이력
`--locked` 첫 실행이 dev-dep 간선 미기록으로 실패(lock 만 offline 갱신, 코드 변경 없음). 그 밖의 실행 검사 실패는 없었다. 수정된 설계 결함은 없다 — 대신 `json_stream` 의 비문자열 키 철자는 IO 기준 039(`null` 버전 키)를 맞추기 위한 공유 소스 확장이다.

### 남은 경계 (미검증·미모델 — 보편 동등성을 주장하지 않는다)
1. **연결/TLS 오류 문구**는 OpenSSL 과 rustls 가 달라 `URLError: <urlopen error ...>` 의 상세 텍스트가 같지 않다(클래스·종료 3·UNRESOLVED 문구는 같다). `NOT_RUN`: 원본과 같은 문구 대조.
2. **타임아웃 모델**: 20초는 읽기·쓰기·연결 설정 각각(전체 20초 아님). 연결 설정 한도는 TCP+프록시 협상+TLS 핸드셰이크 합계에 걸리고(원본은 연산별), 단계 분류(`URLError` 대 `TimeoutError`)는 근사. 실제 20초 초과 시나리오 `NOT_RUN`.
3. **응답 reason phrase**는 표준 문구(`canonical_reason`), 비표준 상태 코드의 서버 문구는 재현 못 함. **리다이렉트**는 최대 10회 후 마지막 3xx 를 `HTTPError`(원본의 무한 루프 문구 아님). `no_proxy` 는 최초 호스트 기준, 프록시 스킴은 `http://` 만 검증, `https_proxy` 가 잘못된 URL 이면 `URLError`. `ALL_PROXY`/`HTTP_PROXY` 는 읽지 않는다(urllib 도 https 에는 읽지 않음).
4. **신뢰 저장소**: `SSL_CERT_FILE`/`SSL_CERT_DIR` 둘 중 하나라도 있으면 그 위치만(OpenSSL 은 다른 쪽 기본값을 유지) — 지원은 Linux 만 검증. 중간 끊김(`IncompleteRead`)은 `OSError` 로 보고(원본은 비-`OSError` 예외) — 단위로 `NOT_RUN`.
5. `http::Uri` 가 거부하는 요청 대상 문자(`<`,`>`,`"` 등)는 원본이 그대로 보내는 경우와 다를 수 있음. 경로 인자는 문자열만 지원(정수 fd API 미지원), 텍스트 환경은 UTF-8 에서만 확인.
6. 실제 상류(api.github.com) 요청은 0건이며 `NOT_RUN`; 주입 opener 30건과 로컬 TLS 는 실제 상류 성공의 증명이 아니다. 성능 측정은 하지 않았다(Root 공식 측정, 타이밍 JSON 없음).
7. 연결되지 않은 부분(Root 소관): `scripts/resolve_guidellm.py` 호출부 교체·런처 2종 설치·`build_rust.sh` 14번째 자체검사·SKILL.md/기록 안내·registry/edges/Terraform 결합. 초안과 목록은 작업 영역의 `HANDOFF.md`.


## R12 교정 — 잘린 본문의 `IncompleteRead`와 CA 소스 독립 선택 (후보 · 정본 적용 전)

두 건은 서로 다른 성격이라 따로 적는다. 앞선 R12 절의 "경계 4"(둘 중 하나라도 있으면 그 위치만 신뢰)와 "ureq `platform-verifier` 기능" 서술은 아래로 **대체**된다. 이전 기록과 증거는 지우지 않았다.

### 1. 실제 실행 FAIL(원본 우선) — 선언 길이보다 짧은 응답 본문
- **관측(Root, 원본 실제 CLI·루프백 TLS)**: `Content-Length` 38 에 21바이트만 보낸 응답 → 원본 종료 1·빈 stdout·마지막 예외 정확히 `http.client.IncompleteRead: IncompleteRead(21 bytes read, 17 more expected)`, 기록 파일 불변. 앞선 후보는 종료 3·`OSError` UNRESOLVED(분류가 달랐다).
- **원인**: ureq 가 연결 조기 종료를 `UnexpectedEof` 로 알려 주는데 `read_failure` 가 일반 `OSError` 로 옮겼다.
- **수정**: `ResponseBody`(`guidellm_release_http.rs`)가 받은 바이트 수와 선언 길이(`Content-Length`)를 세다가 `UnexpectedEof` 를 `IncompleteRead{partial, expected}`(`guidellm_release.rs`)로 바꾼다. `read_failure` 는 이 형만 `IncompleteRead` 예외(클래스 이름 `IncompleteRead`, 메시지 `IncompleteRead(N bytes read, M more expected)`; 길이 선언이 없으면 `IncompleteRead(N bytes read)`)로 되돌리고, `main` 은 이를 `OSError`/`ValueError` 로 잡지 않으므로 traceback 종료 1·`http.client.IncompleteRead:` 접두(CLI `traceback_line`)·기록 쓰기 없음이다. 주입된 읽기 오류(`TimedOut`→`TimeoutError`, 그 밖→`OSError`)와 응답 RAII 정리는 그대로다.
- **청크 본문**: 같은 메커니즘이 청크 중단도 `IncompleteRead(N bytes read)` 로 만든다. `http.client` 는 *완결된 청크*의 바이트만 세지만 여기서는 받은 바이트를 센다(청크가 하나씩 완결되는 경우와 같고, 청크 중간 끊김에서는 다를 수 있음). 원본으로 관측한 것이 아니라 CPython `_read_chunked` 소스에서 특성화한 것이다.
- **검증**: Root 비공개 truncated 픽스처(읽기 전용 import)를 교정 후보에 통과: 종료 1·빈 stdout·traceback·마지막 줄이 Root 가 기록한 원본 줄과 동일·기록 불변·요청 1건(`logs/truncated-private-01.log`, 작업 영역 `evidence-truncated-01/result.json`). 동결 TLS6 도 교정 이진으로 다시 6/6 PASS(`logs/tls-private-02.log`). 등록 검사 `guidellm_release_tls_cases`: 선언 길이 미달(21/38, `--record` 기록 불변)·청크 중단 신규 2건 포함 6건 통과.

### 2. 소스 발견(SOURCE-FINDING) — 부분 CA 재정의가 상대편 기본값을 지웠다
- **관측(Root, 실제 Python `ssl.get_default_verify_paths`)**: `SSL_CERT_FILE` 만 주면 디렉터리는, `SSL_CERT_DIR` 만 주면 파일은 플랫폼 기본값을 유지한다. 후보(`rustls-native-certs` 의 "하나라도 있으면 그 위치만")는 상대편 기본값을 버려 다른 PC 의 기존 HTTPS 를 깰 수 있었다.
- **수정**: ureq 의 `platform-verifier` 기능을 걷어내고(락에서 `rustls-platform-verifier`·jni·windows 계열 등이 빠짐) `RootCerts::new_with_certs` 에 우리가 고른 루트를 넘긴다. 선택은 `select_trust_sources`: **파일 쪽** = `SSL_CERT_FILE` 이 있으면 그 값(빈 값 = 없음), 없으면 플랫폼 기본 파일; **디렉터리 쪽** = `SSL_CERT_DIR`(`:` 목록, 빈 항목 무시, 빈 값 = 없음)이 있으면 그것, 없으면 플랫폼 기본 디렉터리들. 둘 다 재정의하면 그 둘만, 둘 다 없으면 호스트 저장소. 읽기는 `rustls_native_certs::load_certs_from_paths`(주어진 경로의 PEM 만) — 읽을 수 없는 위치는 아무것도 더하지 않는다(OpenSSL 처럼, 잘못된 재정의는 신뢰 없음으로 실패하지 폴백하지 않음). 프로세스 환경·전역 신뢰를 바꾸지 않고, 검증을 끄지 않으며, WebPKI 번들로 넓히지 않고, 외부 프로그램을 실행하지 않는다.
- **환경 주입**: `HttpEnvironment`(프록시·`no_proxy`·`SSL_CERT_FILE`/`DIR` 값·`PlatformTrust`)을 값으로 받는 `HttpsTransport::with_environment`. 운영 CLI 는 `HttpsTransport::from_process()`(프로세스 환경 + 호스트 기본 위치 발견). 플랫폼 기본값은 재정의가 없는 쪽만 발견한다(재정의가 그 쪽을 대체하므로 답이 환경에 의존하지 않는다).
- **의존성**: 락에서 crate 가 *줄었다*(신규 0, 직접 의존 간선 2: `rustls-native-certs 0.8`(default-features 끔)·`openssl-probe 0.2`, 둘 다 이미 트리에 있던 전이 의존). `cargo metadata --offline` 로만 갱신(`logs/cargo-metadata-lock-edge-02.*`, 이전 락 사본 `Cargo.lock.before-trust-sources`).
- **플랫폼 기본값의 한계(정확히)**: Linux GNU 만 지원. 기본 위치는 `openssl-probe` 가 아는 배포판 경로(Debian/Ubuntu `/etc/ssl/certs`·`ca-certificates.crt`, RHEL/Fedora `/etc/pki/...`, SUSE, Alpine, Arch 등)이며 특정 OpenSSL 빌드에 컴파일된 경로가 아니다(이 호스트의 `/usr/lib/ssl` 은 그쪽을 가리키는 링크). 그 밖 위치에 저장소를 둔 호스트는 `SSL_CERT_FILE`/`SSL_CERT_DIR` 로 지정해야 한다. 디렉터리는 비재귀(해시 이름 불필요, 안의 모든 파일을 PEM 으로 읽음), 같은 인증서가 두 곳에 있으면 한 루트로 센다.
- **등록 검사 `guidellm_release_trust_sources`(4)**: 선택 단위(기본/파일만/디렉터리만/둘 다/빈 값/둘 다 빈 값), 읽기 경계(없는 위치는 0 기여·중복 인증서), 그리고 **실제 TLS 핸드셰이크 16판정**: 임시 CA 둘(진짜 서버 CA·다른 CA)과 주입한 "플랫폼 기본" 임시 파일/폴더로 위 각 경우가 신뢰 성립/거부로 갈린다(예: 파일만 재정의 + 기본 디렉터리=진짜 CA → 성공; 디렉터리만 재정의 + 기본 파일=진짜 CA → 성공; 양쪽 재정의가 다른 CA → 호스트 기본이 진짜여도 거부; 빈 파일 재정의는 기본 파일을 되살리지 않음). 기존 양성 TLS(정상·UTF-16 BOM·11MiB·404)와 미신뢰 인증서 거부 검사는 그대로 유지된다. 변형(옛 동작으로 되돌린) 시험은 하지 않았다 — 판정이 갈리도록 구성되었음을 구조로 확인했을 뿐이다.
- **검사 선행조건**: TLS 검사 둘(`guidellm_release_tls_cases`·`guidellm_release_trust_sources`)은 임시 CA 생성에 **`openssl` 실행파일**이 필요하다(없으면 `NOT_RUN` 패닉). 운영 코드는 Rust TLS 로 `openssl` 링크·실행·Python 폴백이 없다. Root 가 이 선행조건을 Terraform 배포 참조에 흡수한다. 이 교정은 호스트 패키지 관리자/전역 설치를 쓰지 않았다.

### 명령·결과 (managed toolchain pin 1.99.1 · offline · locked · 외부 target)
| 항목 | 결과 |
|---|---|
| `cargo fmt -- --check` | PASS (`fmt-check-02`) |
| `cargo build --release` | PASS (`build-corr-final`) · 이진 sha256 `logs/release-binaries-sha256-corr.txt` |
| `cargo clippy --release --all-targets -- -D warnings` | PASS (`clippy-03`) |
| 직접 영향 회귀(`--lib --bins` + pure·io·fetch·cli·api_transport·bench_harness·tls·trust_sources·package_paths) | PASS: 158 통과·0 실패(`test-corr-04`; lib 111) |
| 신규/수정 검사의 첫 실행 | `trust_sources` 의 파일+디렉터리 합산 단위가 FAIL(같은 인증서를 두 곳에 둔 *검사* 가정이 틀림: 로더가 중복을 한 루트로 센다) → 검사만 수정(서로 다른 CA 사용), 제품 코드 변경 없음 (`test-corr-01`→`-03`) |
| 전체 스위트 | 재실행하지 않음(Root 전체 정본 검사 소관). 영향 없는 검사의 이전 유효 결과(전체 334 통과 · 정본 외부 97/42 · 설치 7 · 독립 2)는 변경되지 않은 경로에 한해 유효 |

### 남은 한계
앞선 절의 경계 1~7 중 4번의 신뢰 저장소 서술만 위 2번으로 바뀌었다(나머지 유지). 추가: 청크 `IncompleteRead` 바이트 수는 근사, 플랫폼 기본 위치는 `openssl-probe` 의 목록 범위, macOS/Windows 미검증(미지원), 소스 선택 검사는 옛 동작에 대한 변형 시험이 없다.


## R12 교정 2 — CA 디렉터리는 해시 색인이다 (후보 · 정본 적용 전)

교정 1 의 *디렉터리 읽기*만 이 절이 대체한다(독립 파일/디렉터리 선택·빈 값 의미·`IncompleteRead` 는 그대로). 이전 기록은 지우지 않는다.

### 새 실제 실행 FAIL — 신뢰 범위 확대
- **관측(Root, 원본 대비)**: `SSL_CERT_FILE` 이 명시적 빈 파일, `SSL_CERT_DIR` 에 서버 CA PEM 이 `arbitrary-name.pem` **하나뿐**(해시 항목 없음) → 원본은 인증서 검증 실패로 종료 3·빈 stdout, 교정 1 후보는 그 파일을 신뢰해 종료 0·JSON 출력. 비공개 증거: Root 의 `r12-ca-unhashed-directory-review/`.
- **추가 원본 대조(원본만, 후보 실행 전)**: 같은 인증서가 OpenSSL subject-hash 이름 `6a449899.0` 이면 종료 0, 문법상 유효한 **틀린** 해시 `00000000.0` 이면 종료 3(`r12-ca-hashed-directory-original/`).
- **원인**: 교정 1 이 `rustls_native_certs::load_certs_from_paths(file, dir)` 로 디렉터리 안의 **모든 파일**을 PEM 으로 읽었다. 이는 CA *파일*(번들) 의미이고, OpenSSL 디렉터리는 `<hash>.<n>` 이름으로 색인되는 저장소다 — 검색은 발급자 이름 해시로 그 이름의 파일만 연다.

### 교정
- 새 모듈 `src/openssl_ca_directory.rs`: ① `hashed_entry_index` — 항목 이름은 소문자 16진 8자리 + `.` + 10진수(대문자 해시·`.r0`(CRL)·색인 없음·`.pem` 등은 아님); ② `subject_name_hash` — OpenSSL `X509_subject_name_hash`: DER 인증서에서 subject 를 읽어 속성값을 UTF-8 로(Printable/IA5/Visible/UTF8 은 그대로, T61 은 Latin-1, BMP/Universal 은 UTF-16BE/UTF-32BE 에서) 바꾸고 바깥 공백 제거·안쪽 공백열을 한 칸으로·ASCII 소문자화, 각 RDN 을 DER set(다중 값은 인코딩 순 정렬)으로 다시 써서 바깥 sequence 없이 이은 바이트의 SHA-1(ring `SHA1_FOR_LEGACY_USE_ONLY` — 파일 이름 색인용일 뿐 보안 판정이 아님) 앞 4바이트를 리틀엔디언 u32 로; ③ `load_directory_roots` — 디렉터리를 비재귀로 한 번 읽고 이름이 색인 항목인 파일(심볼릭 링크 따라감: `openssl rehash` 방식)만 PEM 으로 읽어 **인증서의 subject 해시가 이름의 해시와 같은 것만** 신뢰 닻으로 삼는다. 틀린 해시·임의 이름·읽을 수 없는 항목은 아무것도 더하지 않는다.
- `load_trust_roots`(`guidellm_release_http.rs`): **파일**은 그대로 PEM 번들(이름 무관·안의 모든 인증서), **디렉터리**는 위 규칙. 번들 이름이 평범해도 파일 쪽은 유효하고, 같은 파일을 디렉터리 항목으로만 두면 신뢰되지 않는다. 선택(`select_trust_sources`)·빈 값·부분 기본값 의미는 변경 없음.
- 의존성 변경 없음(ring SHA-1 은 기존 직접 의존, DER 는 작은 자체 순회 — 새 crate·fetch 0). 프로세스 환경 변경·재귀 탐색·외부 OpenSSL/Python 호출·검증 비활성·번들 루트 폴백 없음.

### 검증
| 항목 | 결과 |
|---|---|
| 해시 호환(등록 `the_subject_hash_is_openssl_compatible_over_many_name_shapes`) | 실행 시점에 `openssl req`로 만든 자체서명 인증서 11 형태(평범/대소문자/바깥·안쪽 공백/다중 RDN·주 상태·지역/한글·악센트(UTF-8)/반복 CN/다중값 RDN `CN+OU`/긴 값/DC 사슬/탭) 모두 `openssl x509 -hash -noout` 와 일치 — 골든 해시 하드코딩 없음 |
| 실제 TLS 대조(등록 `directory_scope_follows_openssl_hashing`; 같은 생산 로드·HTTPS 경계, `SSL_CERT_FILE=""`) | 거부: 임의 이름 · `00000000.0` · 대문자 해시 · `<hash>.r0` · 색인 없음 · 같은 파일을 디렉터리 항목으로만. 수락: `<hash>.0` · `<hash>.7` · 틀린 항목 사이의 올바른 항목 · rehash 식 심볼릭 링크. 별개로 임의 이름의 CA *파일*(`SSL_CERT_FILE`, 디렉터리 쪽 빈 값)은 수락 |
| 기존 16판정 조합 + 선택 단위 + 읽기 경계 | 디렉터리를 `openssl x509 -hash` 로 올바르게 해시한 항목으로 다시 만들어 유지(기대값을 낮추지 않음). 읽기 경계 단위에 "임의 이름만 있는 디렉터리 = 0 루트" 추가. 이전 버전 시험 파일은 보존: 작업 영역 `drafts/guidellm_release_trust_sources.before-hashed-directories.rs.txt` |
| Root 가 기록한 외부 통제 3종에 생산 로더 적용(`--ignored`, 경로는 환경변수) | `unhashed` 0 루트 · `wrong-hash(00000000.0)` 0 루트 · `matching-hash(6a449899.0)` 1 루트이며 `openssl x509 -hash` 가 `6a449899` — 서버 키가 통제에 없어 이 항목의 TLS 판정 자체는 Root 가 최종 재대조 |
| 단위 | `openssl_ca_directory` 3(항목 이름·정규형·깨진 입력) |
| fmt / build / Clippy | PASS(`fmt-check-04`, `build-dir-final2`, `clippy-05`). 첫 Clippy 가 `manual_is_multiple_of` 2건으로 FAIL → 코드만 수정, 억제 없음 (`clippy-04`→`-05`) |
| 직접 영향 회귀(`--lib --bins` + pure·io·fetch·cli·api_transport·bench_harness·tls(6)·trust_sources(6)·package_paths) | PASS 163·0 실패 (`test-dir-05`) |
| 이전 교정 유지 확인 | 잘린 본문 비공개 증인(`truncated-private-02`: 종료 1·원본과 같은 마지막 줄·기록 불변)과 동결 TLS6(`tls-private-03`: 6/6) 교정 최종 이진에서 재통과 |
| 이진 | `logs/release-binaries-sha256-dir2.txt`(최종). `…-dir.txt` 는 Clippy 수정 직전 빌드, `…-corr.txt` 는 교정 1 빌드 |

### 남은 한계
옛 동작에 대한 변형 시험은 하지 않았다(구성상 갈림). 해시 호환은 위 이름 형태에 대해서만 확인했고 OpenSSL 이 쓰는 다른 문자열 형식(예: 비표준 태그)은 `None`(=그 항목 불신)으로 처리한다 — 항상 불신 쪽이라 신뢰를 넓히지 않는다. 디렉터리 항목은 인증서를 이름 해시와 맞춰 확인하지만 OpenSSL 의 `.r<n>`(CRL) 은 읽지 않는다(해지 검사 범위 밖, 원본과 같은 인증서 신뢰 집합). Linux GNU 만 지원, 이 검사들은 `openssl` 실행파일(Root 가 배포 참조에 흡수)을 요구하고 운영 코드는 요구하지 않는다.


## R12 성능 교정 — 레코드 읽기·쓰기 경로의 사용자 공간 비용 (후보 · 정본 적용 전)

### 관측된 원인 (가설이 아니라 측정; 개인 프로파일 파일은 작업 영역 `perf/`·`logs/perf-*`)
Root 공식 측정: 실제 warmed 읽기+다시쓰기 11건 Python 39.58µs / Rust 50.59µs = 0.78배(순수 1.11배·Bash CLI 8.31배는 유지). 호출 하나를 분해했다.
- 시스템 호출: 이전 8개(openat×2·read×2·write·close×2·statx 1) — `std::fs::read` 의 `statx` 크기 질의 포함. 절단 후 다시쓰기(ext4 truncate+write)의 커널 비용은 호출당 약 12µs 로 Python 과 같은 **작업**(문서 계약: 절단→청크 쓰기→닫기)이라 줄이지 않았다.
- 사용자 시간 호출당 약 11.8µs(`perf stat user_time`): 성분 측정(핫 루프, 256B 레코드) — 읽기 ≈1.9µs(시스템 호출 포함) · 줄바꿈 변환 복사 ≈0.4µs · `loads` ≈1.3–1.9µs · **JSON 덤프 ≈4.7–6.0µs**(청크마다 코드포인트 문자열 `PyStr` 할당 + `strict_utf8` 의 `Vec`/`String` 할당; 이 문서에서 청크 60개 안팎).
- 가설 확인: json_stream 의 구두점마다 `PyStr` 할당과 `FileSink` 청크마다 `Vec` 할당이 실제로 덤프 비용의 대부분이었다. `perf record` 는 이 VM 에서 권한이 없어(`cpu-clock` 불가) 샘플링 대신 성분별 시간과 `user_time`/`system_time`, `strace -c` 로 측정했다.

### 변경 (일반 코드 · 의미 불변)
1. `json_stream.rs`: `ChunkSink` 에 `write_ascii(&str)`(기본 구현 = 기존 `write(&PyStr::new(..))`)를 더하고, 인코더가 구두점·들여쓰기·`null/true/false`/수·`{}`/`[]` 를 **빌린 ASCII 청크**로 넘긴다. 청크 경계는 그대로(스칼라 리스트 항목은 앞 구분자와 한 청크, 키/`: `/값은 각각의 청크, 실수는 프레임 +1 검사 유지). 문자열·키처럼 비ASCII/서러게이트가 가능한 청크는 기존 텍스트 경로다. `FileSink`: ASCII 는 바이트 그대로 `BufWriter` 에, 텍스트 청크는 싱크별 **재사용 scratch 버퍼**에 엄격 UTF-8 로 인코딩(`pyfs::strict_utf8_into`)한 뒤 쓴다 — 실패 시 scratch 를 버리고 기존 `strict_utf8` 와 같은 `EncodeFailure`(위치·문자) 를 낸다. 버퍼(8192)·플러시·닫기·오류 사상은 불변이라 "닫기 오류가 덤프 오류보다 우선" 등 R9 의 우선순위와 부분 쓰기(앞 청크까지만 파일에 남음)가 같다. 전체 문서를 미리 인코딩하지 않고, 원자 교체·캐시·결과 하드코딩이 없다.
2. `pyfs::read_bytes`: `File::open` 후 4 KiB 블록 읽기 루프(끝까지, 필요하면 두 배 성장) — `std::fs::read`/`read_to_end` 의 `statx`(+`lseek`) 크기 질의를 없앤다(오류는 같은 단계·같은 문구: 열기/읽기 실패를 `os_failure` 로 같게 사상, `EINTR` 재시도). 첫 시도로 `read_to_end` 를 써 봤으나 `statx`+`lseek` 가 추가되어 폐기하고 단순 루프로 바꿨다(`logs/perf-strace-c-02/03.txt`).
3. `pyjson::decode_file_text`: 캐리지 리턴이 없으면 줄바꿈 변환용 코드포인트 복사를 건너뛴다(있으면 기존 변환).

### 효과 (진단 측정, 공식 재측정이 아님·같은 11개 IO 입력의 새 사본·`iterations=1` 300라운드·교차 5회)
호출당 중앙값 이전 25.28µs → 이후 20.19µs(−20%), `user_time` 11.8 → 9.0µs, `system_time` 11.8 → 11.6µs(시스템 호출 8 → 7). 이것은 이 호스트에서의 진단값이고 공식 11라운드 값을 대신하지 않는다 — 남은 비용은 대부분 시스템 호출(열기·읽기·절단·쓰기·닫기)이며 사용자 공간 잔여는 `loads`(코드포인트 문자열 할당)와 읽기 래퍼다. 공식 배율 개선은 Root 의 재측정으로만 확인한다.

### 등록 회귀 `guidellm_release_write_path`(8) + 기존 검사
`write_ascii` 가 없는 싱크(기본 구현)와 있는 싱크의 **청크 열이 같음**(구조가 풍부한 문서: 중첩·빈 컨테이너·`null/true/false`·정수·큰 정수·실수·한글/악센트·이스케이프·문자열이 아닌 키 3종) · 파일 덤프가 `dumps_pretty()+"\n"` 과 **바이트 동일** · 8 KiB 버퍼보다 큰 문서와 10만 자 문자열 · 서러게이트 청크에서 멈춘 부분 파일(앞 청크까지 정확히, 오류 문구) · `/dev/full` 에서 작은 문서는 덤프 성공+닫기 `ENOSPC`, 큰 문서는 덤프 중 실패, 인코딩 실패 우선 · `strict_utf8_into` 가 `strict_utf8` 와 같음/실패 시 버퍼 불변 · 0·1·4095·4096·4097·8191·8192·8193·12288·10만 바이트 파일 읽기와 없는 파일/디렉터리/NUL 경로의 오류 문구 · 캐리지 리턴 변환 유무와 UTF-8 오류문. 변경 전 소스로 같은 신규 검사를 돌려 보지는 않았다(새 API 가 있음) — 대신 불변이어야 하는 행동은 기존 등록 검사(R9 보고·IO 19·합성 부분 덤프·하드링크·R9 닫기 우선순위)가 그대로 통과함으로 확인했다.

### 명령·결과 (offline · locked · 외부 target)
`cargo fmt -- --check` PASS(`fmt-check-05`) · `cargo build --release` PASS(`build-perf-final`) · `cargo clippy --release --all-targets -- -D warnings` PASS(`clippy-06`) · **전체** `cargo test --release --no-fail-fast`(공유 경로라 전체 실행) PASS 97 블록·353 통과·0 실패(`test-perf-full-01`; 이전 345 + 신규 8) · 이진 sha256 `logs/release-binaries-sha256-perf.txt`. 수정 전 소스 사본: 작업 영역 `perf/*.before-perf.rs.txt`.

### 범위와 한계
순수 호출·CLI 경로의 코드는 바꾸지 않았다(순수는 이 변경의 영향 밖). 커널 쪽 truncate/쓰기 비용과 `loads` 의 문자열 모델은 건드리지 않았다(의미 보존·재설계 범위 밖). 개선 폭은 진단값이며 Python 대비 배율이 1.0 을 넘는지는 Root 공식 재측정이 판정한다. `perf record` 불가로 함수별 샘플링 프로파일은 없다.


## R12 installed runtime acceptance (Root)

The final installed library/CLI/API passed 97 fixed API and 42 CLI observations (six actual loopback HTTPS cases),
14 build self-tests, fmt, release Cargo (353 passed in 97 result blocks, plus 22 descriptor scenarios; one opt-in
private CA-path replay ignored), and Clippy. The current full distribution invocation passed 131/131. A relocated
Git-index-only copy of 592 files (538 Rust package, 54 source dependencies) built offline with the explicitly reused
node-managed toolchain/cache, passed the same 14 self-tests and the nine registered resolver targets, library and
package-path checks. It copied no build output/cache/authentication; this is not a claim about a different physical PC.
The final shared-writer delta has two actual independent checks: the same 2758-byte hardlink prefix / UnicodeEncodeError
position 6, and the prior report writer's full-disk plus surrogate scenario (1110-byte Markdown, empty stdout, exit 1,
OSError 28 from close takes priority). Source review found no new confirmed defect within that scope.

The final official comparison used all fresh timing records (44 function records and 144 CLI records, zero reused
measurements), eleven alternating rounds of the same fixed inputs. Median Python/Rust: complete pure calls
345.568194/296.319444 ns = 1.166201547x; actual post-one-warm file read+rewrite 40484.272727/45322.181818 ns =
0.893255159x; six real Bash CLI paths, median round mean 35.611760/4.644756667 ms = 7.667088409x. Record-writing
CLI group (two cases) = 7.050252788x, no-record group (four) = 7.765017928x. The small whole-file I/O is still slower
than Python (about 12 percent): keep it as a measured limitation for final Rust-time assessment, not an assertion that
every operation got faster. No first-creation, upstream HTTP speed, LLM, serving or six-profile performance is claimed.
The pre-optimization slower result and the diagnostic A/B measurements remain separate.

The original null-version record is not idempotent between its first and second call: a string key "null" read from
JSON and the next call's None key coexist and are both emitted as "null". The first performance observer incorrectly
expected the first-call file after a warm plus a timed second call. Its failed evidence was retained; the original
saved second-call trees were frozen before the candidate comparison, with no case deletion or original re-execution.
All case IDs, iterations, eleven rounds and timed function boundaries stayed fixed. The final comparison used that
corrected second-call postcondition and checked whole output trees. Prior observer corrections and functional TLS/CA
failures are history, not erased PASS results. The retained Python source is a reference, not a runtime fallback;
this unit does not complete the remaining migration, harness, profile evaluation or Long E2E work.

## R13 토폴로지 해소기(`resolve_topology`) 이행 — 후보 기록 (정본 적용 전)

**상태: COMPLETE — 이번 후보와 자체검사 범위에 한한다.** 성능(정본 계측)·실제 호스트 통합(런처 12결합·`build_rust.sh`·배포 검사 전환)·정본 적용·독립 SAME2 재생·
전체 S1~S7·하네스·6프로필·실제 Long E2E 는 이 기록의 주장이 아니다(Root 소관·미완료). 원본 `resolve_topology.py`(동결 831줄 / 유지 313줄)는 실행하지 않았다(동결 참조만 읽음).
실제 정책 판정자 `topology_parity.py` 는 S7 의존으로 **Python 프로세스 그대로** 호출한다(브랜치·노드·manifest 규칙을 Rust 로 재현하지 않으며 조용한 single 기본값이 없다).

### 산출물과 책임
| 경로 | 책임 |
|---|---|
| `src/resolve_topology.rs` | 원본 4함수: `manifests`(`_manifests`) · `manifest_topology_line` · `run`(`_run`) · `resolve`. 타입: `ResolveContext`(판정자 인터프리터) · `Resolution` · `RunOutput` · `RunLimit`. 검증 불변식: 판정은 판정자의 결과만 옮긴다 |
| `src/captured_process.rs` | R11 `inspect` 의 단일스레드 `OwnedFd`/비차단 poll 포획기를 일반화해 이동. 한 기한이 프로세스 종료와 두 스트림 EOF 를 함께 덮고, 기한에 직접 자식만 kill/reap, 판독자는 함수 반환과 함께 닫힘. `ChildSettings`(cwd·env) 추가. **R11 호출자(120초)는 그대로** |
| `src/dataset_prep.rs`(`LineDecoder`) | `decode_lines` 를 증분 디코더로 분해(동작 동일, 기존 소비자 테스트 유지). 위치 규약: 한 번의 8 KiB raw read 가 단위이고, 나쁜 바이트는 그 read 만 잃는다 |
| `src/resolve_topology_cli.rs` | 원본 argparse 표면(`--repo`·`--self-test`·약어)·종료 0/2/3/5(+미처리 예외 1)·출처 stderr 후 topology stdout. 새 플래그 없음 |
| `src/resolve_topology_api.rs` | `resolve_topology_api evaluate-jsonl/constants/native-help`. 베어 이름 `_manifests`·`_manifest_topology_line`·`_run(argv, timeout=60)`·`resolve(repo, python=None)` · 네트워크·동적 디스패치 없음 |
| `src/resolve_topology_selftest.rs` | 네이티브 `--self-test`: R1~R2 · P1~P5(실제 판정자·git 체크아웃) · S0·S0b · S1~S11(바이트 사본 호출자 6종을 사본 런처·사본 네이티브 실행파일로 실행). 골든 출력이 아니다 |
| `src/bin/resolve_topology{,_api,_bench}.rs` | CLI · API 전송 · 범용 전체 호출 계측기(`ROWS.jsonl ROUNDS ITERATIONS [ID...]`) |
| `src/help_layouts_resolve_topology.rs` | 폭 1..700 usage/help — 원본을 실행하지 않고 같은 파서 정의를 재생해 생성, 기본 폭에서 동결 원본 도움말과 대조 |
| `checks/resolve_topology_*.rs` · `checks/data/resolve_topology/` | 등록 검사 5개 파일(+공용 헬퍼)과 동결 데이터(API 59 · CLI 33 · 원본 selftest 출력) |

공유 소스 변경(최소): `argv.rs`(RESOLVE_TOPOLOGY 표) · `bench_tool_pin.rs`(포획기 이동 · 120초 호출자 유지) · `dataset_prep.rs`(증분 디코더) · `lib.rs` · `Cargo.toml`(bin 3 · test 5). 새 의존성 없음(`Cargo.lock` 불변).

### 보존한 외부 계약과 특이 동작
- `_manifests`: `os.path.join(repo, "output", "*", "manifest.yaml")` 를 **glob 패턴으로** 쓴다(repo 경로의 `[`·`*`·`?` 도 패턴 — 검사로 고정) → `isfile`(끊긴 링크 제외) → 코드포인트 정렬.
- `_manifest_topology_line`: yaml 파서 없이 첫 `topology:` 한 줄(첫 콜론 이후 첫 `#` 앞 · 공백 제거 · 따옴표 제거 · 빈 값 = None) · 첫 일치에서 중단. 읽기는 CPython `TextIOWrapper` 처럼 **8 KiB raw read 단위의 증분 UTF-8·보편 개행**:
  첫 read 안의 일치 앞서 나쁜 바이트가 8200 에 있어도 읽지 않고, 짧은 파일의 나쁜 바이트는(같은 read 이므로) 예외. 경계 걸침(문자 분할·CRLF 분할·EOF 불완전 시퀀스)은 CPython 내장 `open()` 순회로 관측한 값과 대조(비밀 프로브 `probe_reader_boundaries_r13.py`, 판정자 호출 아님).
  OSError(없음·디렉터리·권한·읽기 오류)는 None, UnicodeDecodeError·`embedded null byte` 는 전파.
- `_run`: `subprocess.run(argv, capture_output=True, text=True, timeout)` — 보편 개행·엄격 UTF-8(stdout 먼저)·종료코드(신호 = 음수)·**한 기한이 종료+EOF 포함**(자손이 파이프를 쥐면 기한에서 `(None, "", "Command '[…]' timed out after <시간> seconds")`, 직접 자식 reap) ·
  시간 글자는 호출자가 쓴 수 그대로(`0.2`·`5.0`·`60`) · 시작 실패 OSError·TimeoutExpired 는 값으로, 디코딩·argv 형태 오류는 전파. 부분 출력은 버린다.
- `resolve`: 매니페스트 0장 → 3 · 판정자 파일 없음 → 5 · `[python, parity, "evaluate", "--repo", ABSREPO, "--format", "json"]` 를 기본 60초로 실행 · rc ∉ {0,5} 또는 비-dict → 5 `(rc=…)`+`[-300:]` 코드포인트 꼬리 ·
  `verdict != "PASS"` 또는 `topology` 거짓 → 5 `RED — a / b` · `legs.manifest.status != "ok"` → 3. 잘못된 모양은 원본 예외 그대로(`.get` AttributeError, `str.join` TypeError, `legs_judged` 문자열은 문자별 join, 숫자 topology 의 표시 `True`).
  인터프리터: `python` 인자가 truthy 면 그것(비문자열은 TypeError), falsy 면 컨텍스트 `RESOLVE_TOPOLOGY_PYTHON`(비면 없음 취급) 기본 `python3`.
- CLI 미처리 예외: 종료 1·`Traceback (most recent call last):` 마커와 `Class: message` 마지막 줄(Python 소스 프레임은 재현하지 않음).

### `--self-test` 의존 폐포와 환경
`RESOLVE_TOPOLOGY_REPO_ROOT`(실제 판정자·설정 `…/adversarial-benchmark/config`·의존 스크립트 repeat_axis/bench_preset 8종) · `RESOLVE_TOPOLOGY_SCRIPTS_DIR`(호출자 6종과 `resolve_topology.{py,sh}`·`resolve_topology_api.sh` 바이트 사본) ·
`RESOLVE_TOPOLOGY_RUNTIME_DIR`(다른 배포 네이티브 4종: repeat_axis{,_api}·bench_preset{,_api}). 자기 실행파일: CLI 는 `current_exe()`(이름이 `resolve_topology` 일 때) 아니면 `RESOLVE_TOPOLOGY_BIN` 또는 런타임 폴더, API 는 `RESOLVE_TOPOLOGY_API_BIN` 또는 런타임 폴더.
사본 트리는 기대 릴리스 경로로 복사하고 자식 환경에서 `RESOLVE_TOPOLOGY_BIN/API_BIN`·`REPEAT_AXIS_*`·`BENCH_PRESET_*` 재정의를 제거(사본이 자기 파일을 실행). 미설정 출처는 구성 오류(`ResolveTopologyConfigurationError`)로 끝나며 더 짧은 실행이 아니다.
선행조건(기존): git · bash · python3(판정자) · 테스트 평면의 Flag 게이트 소유자 스텁(사본 트리 안). 서빙·Docker·네트워크 없음.
S0 은 호출자가 **`resolve_topology.sh`** 를 부르는지 본다(옛 `.py` 리터럴이면 실패 — 부정 대조 검사).

### 실행한 검사 (후보 · 관측 근거는 작업자가 후보 밖에 둔 실행 로그 — HANDOFF 참조)
`cargo fmt -- --check` PASS · `cargo build --release --bins` PASS(3 bin 안정) · `cargo clippy --all-targets -- -D warnings` PASS · `cargo test`(전체, `--offline --locked`) **379 PASS · 0 FAIL**(공유 헬퍼 회귀 포함: R11 포획 단위 5 · dataset·bench_tool 전부).
등록 검사: API 59 동결 + 5 · CLI 33 동결(selftest 전체 출력 포함) + 2 · selftest 부정 대조 5(분기 추론 호출자 · `.py` 호출자 · 다른 판정 · 미설정 런타임 · 환경 재정의 무시) · 경계 읽기 6 · 계측기 3.
네이티브 `--self-test` 출력은 동결 원본 출력과 **바이트 동일**(stderr 비어 있음).

### 남은 한계 (사실대로)
- 동결 owner 출력 4종 계측 행은 선언된 로컬 Python 스텁이며 실제 정책·LLM·네트워크 속도 근거가 아니다. 실제 판정자 프로세스 비용은 S7 소유다.
- 지원 입력: 경로는 문자열(정수 = 파일기술자는 `UnsupportedInput`), `_run` 은 문자열 리스트 argv·양수/영/음수 수치 timeout(기한 없음·다른 타입·NaN·표현 불가 큰 수는 `UnsupportedInput` — 임의 축소 아님).
- repo 경로에 NUL 이 있는 glob 은 `ospath::glob`(R8) 상속 한계. 모든 Python 객체 타입·모든 locale 을 증명하지 않는다(UTF-8 텍스트 모드 가정).
- Python 소스 스택 프레임 비재현. 미테스트: Root 가 소유하는 설치 런처·사본 재배치·SAME2 독립 재생·정본 계측.

### R13 교정 — `_manifest_topology_line` 의 메모리는 파일이 아니라 읽기 블록을 따른다 (후보 · 정본 적용 전)

**실제 FAIL(Root 원본 우선 관측):** 18,022,400 바이트·1,638,400 개의 짧은 줄·`topology:` 키 없음 파일을 자식당 주소공간 96 MiB·core 0 한도에서 읽을 때
원본 API 는 종료 0 과 `null`(0.114초), 후보 API 는 SIGABRT(`memory allocation of 64 bytes failed`, 1.251초). 원인은 호출자 `first_topology_line` 이 `LineDecoder.lines`
전체를 유지하고 `examined` 인덱스만 올려, 이미 지나간 모든 줄을 EOF 까지 들고 있던 것이다(`for line in handle` 은 검사한 줄을 버린다). 이전 후보의 "증분 읽기" 주장은
시간·오류 위치에만 맞았고 메모리는 O(파일)이었다 — 이 한계는 이전 기록에서 누락된 것이며 지금 정정한다.

**정밀 수정(호출자만):** 매 read 의 `feed`/`finish` 뒤에 `std::mem::take(&mut decoder.lines)` 로 그 read 가 완성한 줄만 꺼내 순서대로 검사한다. 첫 일치(빈 값 포함) 즉시 중단·현재 read 의 엄격 디코드 우선·불완전 UTF-8/개행
이월/EOF/오류 위치/실패 우선순위는 그대로다. `LineDecoder` 와 `dataset_prep::decode_lines`(전체 줄이 필요한 소비자)는 **변경하지 않았다**. 공개 헬퍼·포획기·selftest·인터페이스 불변, 새 의존성·lock 변경 없음.
긴 한 줄은 원본처럼 그 줄만큼의 메모리가 필요하다(상한을 두지 않았고 크기·사례별 지름길이 없다).

**등록 회귀:** `checks/resolve_topology_reader_cases.rs::many_short_lines_are_read_in_memory_bounded_by_the_block_not_by_the_file` — 같은 모양(18,022,400 B·1,638,400 줄)의 파일을 만들고
API 자식을 `ulimit -c 0 && ulimit -v 98304`(자식 셸 한정, 전역 변경 없음, Python 없음)로 실행해 종료 0·stderr 비어 있음·`null` 을 요구하고, 같은 한도에서 뒤따르는 값(`multi`)과 빈 값 정지도 확인한다.
**판별력 검증:** 예전의 유지형 호출자로 되돌리면 이 검사는 신호 종료로 실패한다(로그 `test-reader-mem-badvariant`), 수정본은 통과한다(`test-reader-mem-1`). 무제한 메모리에서의 "큰 파일이 None" 검사는 나쁜 판을 통과시키므로 쓰지 않았다.
영향 범위 검사: 읽기·API·CLI(33)·selftest 부정 대조·계측기 5개 대상 PASS · fmt PASS · release 3 bin PASS · clippy `-D warnings` PASS. 전체 수트는 이번 변경 범위(호출자·검사 1개)에서는 다시 돌리지 않았다(Root 최종 정본 전체 수트가 뒤따른다).

### R13 교정 — 부정 대조 검사의 설치 배치 가정 (검사만 · 제품 동작 불변)

**실제 FAIL(Root 정본·새 사본 Cargo, 제품 실패 아님):** `the_abbreviated_and_repeated_options_follow_argparse` 가 `CARGO_BIN_EXE_resolve_topology --self` 를 환경 출처를 지운 채 실행하고 종료 1 과
구성 오류를 기대했으나 0 을 받았다. 실행파일이 설치 배치(`…/adversarial-benchmark/rust/target/release`)에 있으면 `SelfTestSources::from_environment` 가 배치에서 출처를 올바르게 도출하므로
환경 제거는 "맥락 없음"을 만들지 못한다. 후보의 외부 cargo-target 에는 그 배치가 없어 가정이 가려져 있었다. 기대 종료값을 0 으로 바꾸지 않았고 제품 소스는 바꾸지 않았다.

**수정(`checks/resolve_topology_cli_cases.rs` 만):** 시험 대상 실행파일을 스크래치 하위 폴더에 `resolve_topology` 로 복사(`std::fs::copy` 가 실행 권한 보존)하고 그 사본을 실행한다. 사본 경로에 `rust` 조상이 없음을 단언(있으면 `NOT_RUN`)하며,
`RESOLVE_TOPOLOGY_REPO_ROOT/SCRIPTS_DIR/RUNTIME_DIR/BIN/API_BIN` 를 모두 제거한다. 같은 종료 1·같은 구성 오류 첫 문구와 `--self` 약어 분기 증명을 유지하고, 반복 `--repo` 검사는 그대로 의미를 갖는다.
재발 방지로 반대편 검사 `an_executable_in_the_installed_layout_finds_its_sources_without_environment` 를 등록했다: 의도된 고정물(`…/adversarial-benchmark/rust/target/release/resolve_topology` 사본 + 판정자 없는 체크아웃)에서
환경 없이 `--self-test` 를 실행하면 구성 오류가 **아니라** 배치로 도출한 체크아웃의 판정자 파일 부재(`FileNotFoundError`, 도출한 경로 포함)로 끝난다 — 설치 배치 발견이 깨지면 이 검사가 실패한다. 조건부 skip 이 아니다.
**검증:** 후보(외부 cargo-target, 배치 없음)와 설치 유사 배치(cargo target-dir 를 `…/adversarial-benchmark/rust/target` 로 둔 비공개 빌드, 실행파일의 `rust` 조상 존재) 양쪽에서 대상 4개 검사 PASS · fmt PASS · clippy `-D warnings` PASS.
한계: 설치 유사 배치는 `debug` 프로파일 폴더이며 `rust` 조상 규칙만 같다(정본 release 경로 자체는 Root 의 실제 실행 소관). 전체 수트는 다시 돌리지 않았다.


## R13 installed runtime acceptance (Root)

The current deployed contract (string paths, list-of-string argv, default sixty-second owner query, UTF-8 environment)
passed 59 original API and 33 CLI comparisons, the same two independent short-timeout/inherited-pipe and actual-owner
(detached HEAD) observations, and source-delta review. This is not approval of every Python-object input: the documented
integer-FD / no-deadline / other timeout / non-list argv limitations remain explicit. The resolver does not infer a
branch/Card topology or clone the parity owner's policy; that actual Python policy owner remains the S7 dependency.
The R11 120-second capture caller and inherited cwd/environment are retained by the extracted capture helper. A rare
try_wait OS failure now names the actual program instead of a fixed docker label; that error path was source-reviewed,
not claimed as an observed runtime equivalence test.

Root's installed build passed 15 native self-tests; fmt, release Cargo (381 passed in 105 result blocks, plus the 22
descriptor scenarios; one optional private CA replay ignored), Clippy, and the installed-launcher 59/33 fixed checks
passed. A full distribution invocation passed 131/131. The Git-index-only relocated copy has 624 files (556 Rust package,
68 source dependencies), offline build15, library114, the five topology targets and package paths, with index bytes equal.
The earlier canonical/fresh Cargo failure was a negative test's missing-layout assumption: an installed executable can
derive sources despite unset environment. Its copied-binary negative still expects the same exit1/configuration error;
a positive installed-layout counterpart reaches the derived missing owner path. Runtime code and binaries were unchanged,
and the valid initial15-selftest builds were reused. This is not a new physical-PC/sub-node deployment claim.

The many-short-lines memory FAIL was preserved: an 18,022,400-byte untagged manifest under child AS96MiB/core0 returned
null in the original, while the retaining reader aborted by SIGABRT. The caller-only completed-line consumption fix passed
that exact saved file/limit with null/exit0 and no original re-execution. The registered equivalent-shape regression fails
the retaining mutant and passes the fixed reader; the shared dataset all-lines behavior was not changed. A very long
single line remains line-proportional memory, as documented. Earlier observer setup/companion-path/fixture-CWD corrections
and the first failing tests remain raw history, rather than erased successful checks.

The first complete official comparison used eleven alternating rounds, zero reused timings: 44 aggregate function
records / 110 individual function records and 144 CLI records. Actual helper file/glob calls25x100: Python6.546296us /
Rust6.312762us = 1.036993950x. Whole resolve with four declared local Python owner-output processes: Python14.36046625ms /
Rust14.70029075ms = 0.976883144x (OPEN S7). Six actual Bash CLI paths: median round mean32.935943667ms /16.808937ms =
1.959430490x; two no-owner paths4.527652227x, four owner paths1.778619583x. These are no synthetic purefunction measurements.
The owner fixture reads a response and writes its call record in the inherited cwd, so each whole-owner case runs in its
own scenario cwd (prepared outside the timer); four complete call timers are summed/divided outside the timed regions.
All four IDs/one iteration/eleven rounds stayed fixed, both sides match, and original after_api trees (capture write
included) were checked. The failed initial batched-CWD warm preparation was retained and corrected before candidate timing.
The local fixture/process cost is not real policy, LLM, network, serving or profile speed; review the remaining S7 owner
cost explicitly after that dependency's port. The source connection ledger is not line coverage or Python-zero proof.
The remaining S1-S7, harness, six-profile and actual Long E2E work is still incomplete.

## R14 `full ⊇ lite` 검증기(`conformance_full_superset_lite`) 이행 — 후보 기록 (정본 적용 전)

**상태: COMPLETE — 이번 후보와 아래 자체검사 범위에 한한다.** 성능(정본 계측)·실제 호스트 통합(Bash 런처 2·`build_rust.sh` 16번째 검사·배포 검사 131·`selftest_sweep_repeats` 호출부·간선·SKILL)·정본 적용·
독립 SAME2 재생·전체 S1~S7·하네스·6프로필·실제 single4/multi2 Long E2E 는 이 기록의 주장이 아니다(Root 소관·미완료). 원본(현재 248줄 `conformance_full_superset_lite.py`, R8/R9 이후 설치된 Rust 소유자를 검사하는 판)은
실행하지 않았다(동결 참조만 읽음). 원본 831→184줄 시절의 정적 골든·링크된 라이브러리 호출로 되돌리지 않았고, 실제 `bash` 런처와 그 뒤 실행파일을 매번 자식 프로세스로 부른다.

### 산출물과 책임
| 경로 | 책임 |
|---|---|
| `src/conformance.rs` | 컨텍스트(`Context`: 스크립트 디렉터리 `SDIR` · env `CONFORMANCE_SCRIPTS_DIR` 우선, 없으면 `<skill>/rust/target/release/<bin>` 설치 배치에서 도출, 그 밖의 위치는 컨텍스트 없음) · `Raised`(전송이 보고하는 class·message + `repr()` — `except Exception as exc … repr(exc)` 의 근거) · `read`(UTF-8 `replace`·보편 개행·비파일 `None`) · `read_optional_strict`(엄격 UTF-8 + `OSError`만 빈 문자열) · `emit`(`_emit`) · 점검이 의존하는 Python 의미(`py_equals`·`subscript`·`dict_get`·`find_position`=문자 위치·`splitlines`·`rstrip`·`sets_key_from_built`) |
| `src/conformance_native.rs` | `native_value`(실제 `bash bench_reporting_api.sh evaluate-jsonl`, stdin 한 줄, 600자 꼬리 RuntimeError) · `native_lite_certificate_gates`(실제 `publish_benchmark_record.sh` 3회: beta 거부 · gamma 거부 · 양성 대조) · `LiteMetrics`(원본 `lite_metrics_native.py` **어댑터를 실행하지 않고** 같은 프로토콜로 `lite_metrics_api.sh constants/evaluate-jsonl` 을 직접 호출: 같은 요청 모양·`json-values-v1` 검사·`_raise` 의 builtin 예외 매핑·`KeyError` 리터럴) |
| `src/conformance_checks.rs` | 원본 `main` 의 점검 41개를 같은 순서·같은 문자열로. 단계별 `try/except Exception` 경계(게이트·렌더 단계, 판정기 적재·D1 단계)와 그 밖의 전파를 보존 |
| `src/conformance_cli.rs` · `src/bin/conformance_full_superset_lite.rs` | CLI: **인자를 읽지 않는다**(`--help` 포함, 새 플래그·selftest 없음). 종료 0/1, 미처리 예외는 `Traceback…`+`Class: message` 마지막 줄·종료 1. 컨텍스트 없음 = 종료 2(이 포팅 고유) |
| `src/conformance_api.rs` · `src/bin/conformance_full_superset_lite_api.rs` | `evaluate-jsonl`(연산 `read`·`_emit`·`native_value`·`native_lite_certificate_gates`·`main`) · `constants`(LITE_METRIC_KEYS·CERT_LITE_FIELDS) · `native-help`. 출력하는 연산(`_emit`·`main`)은 **예외로 끝나도** 그때까지 찍은 `stdout` 필드를 싣는다. 이웃 `Signature`/`PyValue` 전송 재사용, 임의 exec 없음 |
| `src/bin/conformance_full_superset_lite_bench.rs` | 범용 계측기 `ROWS.jsonl ROUNDS ITERATIONS [ID...]`(R13 과 같은 출력 `{cases, iterations, rounds_ns}`). 행 준비·기대값 파싱은 타이머 밖, 전체 호출(바인딩·읽기·자식 프로세스·출력 생성·반환·해제)은 안. 워밍 호출이 `expected_json`(값)·`expected_stdout`(출력 전체)을 검증하고, 예외·불일치는 타이밍 없이 종료 1 |
| `src/captured_process.rs`(공유, 최소 변경) | `ChildSettings.input`(`subprocess.run(input=…)`): stdin 을 비차단 파이프로 **하나의 통신 루프 안에서** 출력 두 스트림과 함께 `PIPE_BUF` 단위로 쓰고 닫는다(자식이 일찍 닫으면 EPIPE 는 조용히 종료). 루프는 **대기 중 입력이 있는 동안 계속**된다(출력이 둘 다 EOF 가 된 뒤에도 — 아래 교정). 기한 없는 호출용 `run_captured_unbounded`(같은 루프·마지막 종료 대기는 블로킹 `wait`, 폴링·지연·분리 스레드 없음). 기존 유한 호출자(120초/60초)는 그대로. `resolve_topology_selftest.rs` 의 리터럴 1곳에 `input: None` 추가 |
| `checks/conformance_*.rs` · `checks/data/conformance/` | 등록 검사 4파일(+공용 헬퍼)과 동결 데이터(헬퍼 24 · 전송 10 · 정상 출력 41줄) |

공유 소스 변경: `captured_process.rs`(입력·무기한 경로) · `resolve_topology_selftest.rs`(리터럴 1필드) · `lib.rs` · `Cargo.toml`(bin 3 · test 4). 새 의존성 없음(`Cargo.lock` 불변).

### 보존한 외부 계약과 특이 동작
- **D1 hostdead 버그를 그대로 둔다.** 점검이 사전(dict) 상세를 들고 실패하면 원본 `_emit` 은 `"  — " + detail` 에서 `TypeError: can only concatenate str (not "dict") to str` 로 끝나며, 그 전에 찍은 줄은 stdout 에 남고 요약 줄은 없다(종료 1). 자동 pretty-format 으로 고치지 않았다. 문자열이 아닌 truthy 상세는 일반적으로 같은 오류(`int`·`list` …).
- `_emit` 은 행을 읽는 즉시 찍는다: 짧은 행 `ValueError("not enough values to unpack (expected 3, got N)")`, 긴 행 `"too many values to unpack (expected 3)"`, 앞선 줄은 출력된 채. `good` 은 파이썬 진리값, `name` 은 `str()`(`None` → `None`), `detail` 이 falsy 면 무시.
- `native_value`: `bash <SDIR>/bench_reporting_api.sh evaluate-jsonl` · 요청 `json.dumps({"id":"conformance","operation":…,"arguments":…})+"\n"` · 120초 · 종료코드≠0 또는 stderr 비어 있지 않음 → `RuntimeError("native reporting call failed: " + stderr[-600:])` · 엄격 UTF-8·보편 개행(stdout 먼저) · JSON 오류·`.get` AttributeError·`RuntimeError("native reporting returned no text: %r")` 를 원본 순서대로.
- 점검 결과의 `repr(exc)`: 구조화된 오류 인자를 가진 종류는 원본 repr 그대로(`UnicodeDecodeError('utf-8', b'…', start, end, 'reason')` · `TimeoutExpired([...], 120)` · `FileNotFoundError(2, 'No such file or directory')`).
- 점검의 위치 표기 `gate@N level@M` 은 **문자 위치**(UTF-8 바이트 아님)다 — 한글이 앞서므로 둘이 다르다(동결 late-gate 의 `71824`/`30912` 와 같은 방식으로 계산).
- 정규식 `"KEY":\s*_built\.get`(`\s` = `str.isspace`: Rust `White_Space` + U+001C–001F)는 손으로 구현했고 단위 검사로 고정. 원본이 먼저 시도하는 더 긴 정규식은 이것의 부분집합이라 결과가 같다.
- 선택 읽기(`campaign_init.py`·`recipe.py`·분류기 Rust 소스 3종)는 엄격 UTF-8 이고 `OSError` 만 빈 문자열이다. 디코딩 오류는 전파(원본: 보고서가 한 줄도 출력되기 전에 `UnicodeDecodeError`, 종료 1).
- `read` 의 `name` 이 절대경로면 `os.path.join` 처럼 `SDIR` 를 무시하고, 문자열이 아니면 `join() argument must be str…` TypeError 다.

### 환경과 의존 폐포 (실행 시)
`CONFORMANCE_SCRIPTS_DIR`(스크립트 디렉터리; 점검은 그 옆의 `fixtures/`·`rust/src/`·이웃 스킬 두 파일도 읽는다) · `bash` · 스크립트 디렉터리의 런처 3종(`bench_reporting_api.sh`·`publish_benchmark_record.sh`·`lite_metrics_api.sh`)과 그 뒤 설치된 릴리스 실행파일 3종
(런처 자체의 `BENCH_REPORTING_API_BIN`·`PUBLISH_BENCHMARK_RECORD_BIN`·`LITE_METRICS_API_BIN` 재정의는 자식에게 그대로 전달된다). 설치 배치 밖의 실행파일에 env 도 없으면 종료 2. 자동 빌드·설치·Python 대체 경로 없음.
`LiteMetrics` 호출은 원본 어댑터처럼 **시간 제한이 없다**(발명한 상한 없음 — 아래 교정). 원본 `lite_metrics_native.py` 는 **존재 점검(정적)** 만 한다 — 실행하지 않는다. Python 은 이 검증기의 런타임에 필요 없다(점검 대상 소스가 Python 일 뿐).

### 실행한 검사 (managed toolchain `rustc 1.99.1` · 후보 크레이트 · `--offline --locked` · 외부 target · 로그는 작업자의 별도 실행 로그 폴더)
| 항목 | 상태 | 근거 |
|---|---|---|
| `cargo fmt -- --check` | PASS | exit 0 |
| `cargo build --release --bins` | PASS | exit 0 · 3 bin 안정 |
| `cargo clippy --all-targets -- -D warnings` | PASS | exit 0 |
| `cargo test --release --no-fail-fast`(전체, 최종 실행) | PASS | 112 대상 · **433 통과 · 0 실패** — 새 점검 포함: lib 단위 15(`conformance` 10 · `conformance_native` 5) · bench bin 단위 4 · 등록 검사 33 |
| 등록 검사 `conformance_helper_cases` | PASS 9 | 동결 헬퍼 24(`_emit` 14 · `read` 10) · 전송 10 · 디코딩 8 · 절대경로·권한·엄격 읽기·`main` 조기 종료·시작 불가 런처 |
| 등록 검사 `conformance_api_transport` | PASS 9 | 동결 34 의 봉투 · 예외 시 stdout 접두 보존 · 바인딩 TypeError 5종 · JSON 형식 · constants/help/usage · 실제 런처 사본 설치 |
| 등록 검사 `conformance_cli_cases` | PASS 10 | 정상 41/41 바이트 동일 · 무시 인자 4종 · 누락 소스 · 늦은 게이트 · 항상 발행 · 깨진 프로토콜 · D1 hostdead(TypeError) · 컨텍스트 없음 종료 2 · 설치 배치 도출 · 디코딩 불가 소스 |
| 등록 검사 `conformance_bench_harness` | PASS 5 | 검증된 워밍 뒤 타이밍 · 전체 검증기 계측(실제 자식 프로세스) · 기대 불일치/예외 거부 · 사용법 |
| 동결 원본 재생(`logs/frozen_replay_final.log`) | PASS | CLI 7/7(stdout 바이트·종료·마지막 stderr 줄) · API 34/34(값·예외 class/message·stdout) · 불일치 0 |
| 원본 문자열 상수 대조(비공개 프로브) | PASS | 150 상수 중 12 불일치 = 전부 `%` 서식 템플릿(Rust 서식으로 재표현) |
공유 모듈(`captured_process`) 영향 검사: `bench_tool_*`·`resolve_topology_*`·R11 포획 단위 전부 위 전체 수트에 포함되어 통과.

### 실패·수정 이력과 알려진 간헐 실패 (사실대로)
- 내 새 검사 1건이 실제 FAIL: 복사한 실행파일의 `ETXTBSY`("Text file busy" — 다른 스레드가 fork 한 자식이 쓰기 핸들을 exec 전까지 들고 있음). **내 검사를 고쳤다**: 같은 파일의 하드링크로 배치(쓰기 핸들 없음)하고 실행은 유한 재시도(20ms×100). 연속 24회 재실행 0 실패.
- **기존 R13 검사 `checks/resolve_topology_cli_cases.rs` 의 2개**(`the_abbreviated_and_repeated_options_follow_argparse` · `an_executable_in_the_installed_layout_finds_its_sources_without_environment`)는 같은 `ETXTBSY` 로 **간헐 실패**한다
  (이 호스트에서 단독 12회 중 5회 실패, 같은 오류로 확인; 병렬 테스트 스레드가 실행파일을 복사하는 동안 다른 스레드가 fork). R14 변경과 무관한 기존 결함이며 이 후보는 **그 파일을 수정하지 않았다**(Root 소관). R13 검사까지 도달한 전체 수트 3회 중 앞선 2회에서 이 R13 검사가 실패했고(그중 1회는 위의 내 ETXTBSY 검사도), 수정 후 최종 실행은 0 실패였다 — 즉 R13 검사는 여전히 간헐적이다.
  권고 수정(검사만, 제품 불변): 복사 대신 하드링크 또는 위와 같은 ETXTBSY 유한 재시도.

### 남은 한계 (미검증·미모델 — 보편 동등성을 주장하지 않는다)
- 지원 범위: 배포 UTF-8 텍스트·문자열 경로. 모든 Python 객체·모든 locale 을 모델링하지 않는다(`_emit` 인자는 JSON 값: 리스트/튜플/문자열/dict 의 풀이만, 그 밖은 `TypeError`).
- `KeyError` 리터럴은 인용 문자열·정수·`None`/`True`/`False` 만 해석한다(그 밖의 리터럴은 메시지 자체를 인자로 — 원본은 `ast.literal_eval` 전체).
- (정정) 이전 기록의 "8 KiB 를 넘는 파일의 `UnicodeDecodeError` 위치는 다를 수 있다"는 **근거 없는 한계였고 삭제한다.** `open(path, encoding="utf-8").read()` 는 파일 전체를 한 번에 디코드하므로
  위치는 파일 전체 기준이다(CPython 으로 20 000·100 000 바이트에서 확인) — R13 의 줄 단위 읽기 규약과 혼동했다. 등록 검사 `strict_read_errors_report_whole_file_positions_for_large_files`.
- (정정) 이전 기록의 "lite 호출은 1년을 한도로 둔다"는 **발명한 상한이었고 삭제했다** — 아래 교정.
- 호스트 임시 디렉터리 이름 규칙은 `tempfile` 과 같은 접두·8자 형태이나 난수원은 다르다(관찰 계약 아님).
- 성능은 측정하지 않았다(Root 의 사전 고정 계측 소관). 계측기는 예외·불일치 시 타이밍을 내지 않는다.
- Python 소스 스택 프레임 비재현. Root 소유 미연결: 런처 2·`build_rust.sh`·배포 검사·호출부·간선·SKILL.

### R14 교정 — 입력은 출력이 닫힌 뒤에도 서비스한다 · 기한 없는 경로 · 검사 설정 (후보 · 정본 적용 전)

**실제 FAIL(Root 원본 우선 관측, 첫 빌드):** 전체 `native_value` API 가 자식(stdout·stderr 를 닫은 뒤 stdin 을 EOF 까지 읽음)을 부를 때, 원본은 262 226 바이트 요청을 모두 전달하고(0.0042초) `JSONDecodeError(Expecting value: line 1 column 1 (char 0))` 로 끝났으나
첫 Rust 빌드는 65 536 바이트(파이프 용량)만 전달하고 Root 의 외부 3초 가드에 SIGTERM(종료 −15)로 끝났다. 이 기록은 Root 의 원시 결과를 바꾸지 않는다.

**원인(소스):** `captured_process::capture_until` 의 통신 루프 조건이 `while stdout.open || stderr.open` 이라 대기 중 stdin 을 계산에서 뺐다. 출력 두 스트림이 EOF 가 되면 루프를 떠나 자식 종료를 기다렸고, stdin 은 열린 채 더는 쓰이지 않았다.
**수정:** 루프 조건이 대기 중 입력을 포함한다(`|| stdin.is_some()`). 같은 한 번의 `poll`·같은 기한·같은 `PIPE_BUF` 단위(공정성)·입력 완료 시 닫기·빈 입력(즉시 닫음)·일찍 닫은 자식의 EPIPE(조용히 종료)·`None` 일 때 상속 stdin 은 그대로다.
제품 기한(120초/60초)을 줄이거나 지연·특정 사례 분기를 두지 않았다. 마지막 종료 대기(유한 경로)는 기존 폴링 그대로.

**기한 없는 경로(Root 검토 반영):** 원본 `LiteMetrics` 호출은 `timeout` 이 없다. 365일 상한을 삭제하고 `run_captured_unbounded` 를 두었다 — 한 통신 루프(입력+두 출력)가 기한 없이 `poll` 하고, 모든 파이프가 끝난 뒤의 종료 대기는 블로킹 `child.wait()` 다(폴링 루프·분리 판독 스레드·1년 대기 없음).
유한 공개 함수와 호출자(`run_captured_within`/`run_captured_in`, 120초/60초)는 같은 본문(`run_child`)을 `Some(한도)` 로 쓰며 동작이 같다. R13 `_run(None)` API 지원 범위는 넓히지 않았다.

**등록 회귀(짧은 한도·실제 `/bin/sh` 자식):** `captured_process::tests` 4개 추가 — ① `input_is_still_served_after_the_child_closed_both_outputs`(자식이 두 출력을 닫고 1 MiB 비반복 입력을 EOF 까지 읽음 → 전체 바이트·순서·종료 0, 10초 한도 안에서 5초 미만) ② 같은 자식이 읽지 않으면 짧은 한도(0.3초)에서 TimeoutExpired·kill·reap ③ 빈 입력·일찍 닫음·에코(입출력 교차) ④ 무기한 경로(출력 닫고 읽은 뒤 0.4초 더 살다 종료 5 — 대기가 끊기지 않음, 에코 포함).
API 수준: `native_value_delivers_a_large_request_to_a_child_that_closed_its_outputs`(Root 시나리오와 같은 모양: 262 144 자 payload → 자식이 받은 바이트가 요청 전체와 같고 `JSONDecodeError`) · `the_lite_metrics_call_without_a_limit_writes_waits_and_answers`(40만 자 요청·느린 답·한도 없음).
**판별력:** 루프 조건을 옛 `while stdout.open || stderr.open` 으로 되돌린 변형은 ①이 FAIL(`TimeoutExpired … after 10 seconds`, 로그 `s2_mutant_retaining_loop`), 수정본은 PASS(`s2_unit`). 변형은 검사 후 원복했다(원본 사본은 후보 밖). API 수준 검사는 변형에서 120초(제품 한도) 후에야 실패하므로 변형 실행은 단위 검사로만 했다.

**Bench:** 이제 모든 행이 `expected_json` 과 `expected_stdout` 을 **둘 다** 가져야 하고(없거나 문자열이 아니면 준비 단계에서 거부), 워밍 호출이 전체 값과 전체 출력(출력 없는 연산은 `""`)을 모두 대조한 뒤에야 타이밍한다. 타이머 안의 범위(바인딩·호출·자식·출력 생성·반환·해제)는 불변.

**R13 검사 설정 변경(Root 승인 · `checks/resolve_topology_cli_cases.rs` 만 · 제품 불변):** 원인은 진짜 시험 설정 경합이다 — `std::fs::copy` 로 만든 실행파일을 곧바로 실행할 때, 병렬 시험의 다른 스레드가 fork 한 자식이 exec 전까지 쓰기 핸들을 상속해 쥐고 있으면 `ETXTBSY`.
변경: `isolated_cli` 와 설치 배치 검사가 복사 대신 `place_executable`(하드링크 — 쓰기로 연 파일이 없음, 다른 파일시스템이면 복사)을 쓰고, 그 두 실행만 `output_after_busy` 로 감싼다(**시작이 ETXTBSY 일 때만** 20ms×100 유한 재시도; 다른 OS 오류는 즉시 실패, 시작된 프로그램은 종료 상태와 관계없이 다시 실행하지 않음). 기대 문구·종료값·4개 사례 수 불변. 이전 간헐 실패 이력(단독 12회 중 5회, 전체 수트 3회 중 2회)은 위 기록에 보존했다.
확인: 변경 후 단독 8회 8 PASS(`s2_r13_setup_confirm`).

### R14 교정 후 실행한 검사 (managed `rustc 1.99.1` · offline · locked · 외부 target)
`fmt -- --check` PASS · `clippy --all-targets -- -D warnings` PASS · `build --release --bins` PASS(3 bin 안정) · `cargo test --release --no-fail-fast` **112 대상 · 441 통과 · 0 실패**(`s2_test_full`; 새 등록 검사 37 = 12+9+10+6, bench bin 단위 4, `captured_process` 단위 10)
· 동결 원본 재생 CLI 7/7 · API 34/34 불일치 0(`frozen_replay_stdin_fix.log`). Root 의 stdin 실제 사례 재생과 독립 검토는 Root 소관(이 후보가 대신 주장하지 않는다).

### R14 교정 3 — `SystemExit` 은 `Exception` 이 아니다 (후보 · 정본 적용 전)

**실제 FAIL(Root 원본 우선·`r14-system-exit-reference`):** 선언된 lite 프로토콜 더블이 유효한 constants 뒤에 `kind: exception, type: SystemExit, message: declared-protocol-exit` 를 돌려주면,
`lite_metrics_native._raise` 가 `SystemExit` 를 명시적으로 허용하고 원본 `main` 의 `except Exception` 은 이를 잡지 않으므로 원본은 **종료 1·stdout 비어 있음·stderr `declared-protocol-exit\n`** 이다.
직전 후보(v2)는 모든 `Raised` 를 점검 상세로 접어 종료 1·stdout 2725 바이트·stderr 비어 있음·36/37 FAIL 이었다. 이 원시 결과는 그대로 보존된다.

**수정(소유 클래스·가드·CLI 분기만):**
- `Raised::is_system_exit()` — 잡히지 않는 클래스의 이름을 가진 단일 판정.
- `conformance_checks::caught_as_exception()` — 원본의 두 `except Exception` 경계(게이트·렌더 단계 / 판정기 적재·D1 단계)가 이 한 도우미를 쓴다. 보통 예외는 기존대로 점검 상세(`repr`), `SystemExit` 는 `Err` 로 `main` 밖으로 전파되어 **보고서가 한 줄도 출력되지 않는다**.
  모듈 적재(`import`)의 `except (RuntimeError, ValueError, KeyError, TypeError)` 는 원래 `SystemExit` 를 포함하지 않아 그대로 전파.
- CLI: `SystemExit` 는 traceback 이 아니다 — 문자열 사유 + 개행을 stderr 에, 종료 1. **사유는 문자열이다**: `"0"`·`"7"`·`""` 도 숫자 상태가 아니라 문자열 사유(빈 문자열은 빈 줄)이며 다른 프로그램의 숫자 파싱 `exit_status` 를 쓰지 않는다.
- API: `main` 은 라이브러리에서 프로세스를 끝내지 않고 `{kind: exception, type: SystemExit, message, stdout}` 봉투(그때까지 찍은 stdout 접두 포함)를 낸다.
변경하지 않음: 보통 예외의 점검 처리(ImportError·JSONDecodeError 등) · 부분 stdout · D1 TypeError · 정상 41 · 동결 기준.

**등록 회귀:** `conformance_cli_cases::a_reported_system_exit_ends_the_checker_with_its_string_reason`(선언 더블 형태 그대로: 유효 constants → SystemExit 레코드; 사유 `declared-protocol-exit`·`0`·`7`·`""` 각각 종료 1·stdout 비어 있음·stderr = 사유+개행) ·
`a_reported_ordinary_exception_is_still_a_failed_check`(같은 더블의 `ValueError` 는 보고서·`FAIL E 판정기 적재·픽스처 판정  — ValueError('…')`·stderr 비어 있음) ·
`conformance_api_transport::a_reported_system_exit_is_an_exception_envelope_not_a_process_exit`(API 프로세스는 0 으로 끝나고 봉투가 `SystemExit`/`0`/stdout `""`).
**검사:** `fmt --check` PASS · `clippy --all-targets -D warnings` PASS · `build --release --bins` PASS · 대상 4개 등록 파일과 lib `conformance*` PASS(CLI 12 · helper 12 · bench 6 · API 단위 15 lib) · 동결 CLI 7/7 재생 불일치 0 ·
Root 의 SystemExit 사례를 선언 더블(`LITE_METRICS_API_BIN`)로 재생: 종료 1·stdout 비어 있음·stderr `declared-protocol-exit\n` 일치(Root 의 정식 재생이 따른다). 전체 Cargo 는 다시 돌리지 않았다(이전 441/0 은 이번에 바뀐 대상을 제외하고 유효, Root 전체 검증이 뒤따른다).
**한계:** 프로토콜 `message` 는 문자열 범위다 — 비문자열 `message` 의 `SystemExit(int)` 종료 상태 의미는 모델링하지 않는다. 기록상 8200 바이트 엄격 읽기는 Root 가 실제 PASS 확인(`r14-large-strict-reference`), 소스 동작 불변.

### R14 교정 4 — 음성 대조 검사의 설치 배치 가정 (검사만 · 제품 동작 불변)

**실제 FAIL(Root 정본 설치 Cargo · 새 사본 Cargo, 제품 실패 아님):** `without_a_scripts_directory_nothing_is_checked` 가 `CONFORMANCE_SCRIPTS_DIR` 를 지운 채 `CARGO_BIN_EXE_conformance_full_superset_lite` 를 실행하고 종료 2 를 기대했으나 0 을 받았다.
`CARGO_BIN_EXE` 가 `…/rust/target/release` 안에 있으면(설치 배치) 실행파일의 설치 배치가 스크립트 디렉터리를 올바르게 공급하기 때문이다. R13 에서 이미 교정한 것과 같은 이식성 가정이며 런타임 결함이 아니다
(후보의 외부 cargo-target 에는 그 배치가 없어 가려져 있었다). 기대 종료 2·정확한 진단·무출력은 그대로이고 기대값을 0 으로 바꾸지 않았다.

**수정(`checks/conformance_cli_cases.rs` 의 검사 1개만):** 대상 실행파일을 스크래치 트리의 `rust` 조상이 없는 자리에 하드링크(다른 파일시스템이면 복사)로 두고(`rust` 조상이 있으면 `NOT_RUN`), 환경에서 `CONFORMANCE_SCRIPTS_DIR` 를 지운 채 그 격리본을 실행한다.
설치 배치 양성 검사 `a_copy_of_the_binary_inside_an_installation_finds_its_scripts` 는 그대로(새 대응 검사 없음). 런타임·Core·CLI·API·bench·환경·게이트 무변경.
**검증:** 후보(외부 cargo-target) `conformance_cli_cases` 12 PASS(`s4_cli`) · 설치 유사 배치(cargo target-dir 를 `…/adversarial-benchmark/rust/target` 로 둔 비공개 빌드 — `CARGO_BIN_EXE` 의 `rust/target/release` 조상 존재, 정확히 이 검사가 실패하던 조건) 에서도 12 PASS(`s4_cli_sim_installed`) ·
fmt PASS · clippy `-D warnings` PASS. 전체 Cargo 는 다시 돌리지 않았다. 제품 릴리스 3 bin 은 v3 hold 와 바이트 동일(sha256 불변).

### R14 교정 5 — 같은 프로세스·같은 컨텍스트의 반복 `main()` 은 메트릭 모듈을 다시 import 하지 않는다 (후보 · 정본 적용 전)

**실제 불일치(Root 원본 우선·`r14-import-cache-reference`):** 원본은 한 프로세스에서 `main()` 을 두 번 불러도 `import lite_metrics_native` 가 `sys.modules` 로 한 번만 실행되어 실제 메트릭 소유자의 `constants` 를 **1회** 얻는다.
v4 는 JSONL 로 `main` 을 두 번 부르면 `constants` 를 **2회** 얻었다(전체 값·stdout 은 41/41 로 같고 추가 자식 프로세스·부수효과만 달랐다). 이 원시 결과는 그대로 보존된다.

**수정(상태 소유와 import 경로만):** `Context` 가 `ModuleCache`(원본의 `sys.modules` 에 해당)를 `Arc` 로 든다 — 같은 컨텍스트와 그 복제는 한 상태를 공유하고, 따로 만든 컨텍스트는 빈 상태로 시작한다.
`LiteMetrics::load(&ModuleCache)` 는 **성공한** import 의 `LiteBindings`(상수)만 보관한다. 실패한 import 는 기억하지 않는다(파이썬은 import 에 실패한 모듈을 `sys.modules` 에서 지우므로 다음 import 가 다시 실행된다). 락은 import 동안 잡는다(import 락과 같은 취지).
`main` 은 `load(context.modules())` 를 쓰고, 기존 `import()`(캐시 없음)는 그대로 둔다.
**캐시하지 않는 것:** 오류 · 메트릭 연산 결과 · 파일 내용 · 점검 전체 · 출력 · 부수효과. 매 `build`/`judge_lite`/`read_lite_verdict`/`_load_json` 은 매번 실제 소유자를 호출하고, 보고서·발행·소스 읽기도 매번 실행된다.
벤치·페이로드·경로·ID 별 분기, 결과 조회, 워밍업 분기 없음(계측기의 컨텍스트는 프로세스당 한 번 만들어지므로 같은 의미가 자연히 적용). 단일 CLI `main` 은 여전히 콜드 import 로 `constants` 를 1회 묻는다. 오류·`Exception`/`SystemExit` 가드·stdout·데이터 트리·`None` 기한·stdin 공정성 무변경, 새 의존성 없음.
**범위:** 파이썬은 모듈 이름으로 캐시하므로 한 프로세스에서 서로 다른 스크립트 디렉터리가 같은 모듈을 공유한다 — 이 포팅은 컨텍스트 단위이며 그 일반 동일성은 모델링하지 않는다.

**등록 검사(`checks/conformance_import_cache.rs`, 4):** 실제 설치 소유자 바이너리에 위임하는 **세는 래퍼**(모든 호출의 첫 인자를 기록하고 위임, 결정 모의 없음).
① 같은 프로세스에서 `main` 2회 → 두 전체 값(0)·전체 stdout(41/41) 일치, `constants` 1회, `evaluate-jsonl` 호출 수는 단일 `main` 의 정확히 2배(연산이 매번 소유자로 감) ·
② 첫 `constants` 가 실패하는 래퍼로 `main` 3회 → 첫 `main` 은 상태 1·`FAIL E 판정기 적재·픽스처 판정  — ImportError('lite metrics native transport failed (3): constants unavailable')`, 둘째는 다시 import 하고 41/41, 셋째는 캐시 사용; `constants` 총 2회, `evaluate-jsonl` 은 통과한 두 `main` 몫 ·
③ 단일 CLI `main` → `constants` 1회, 전체 stdout 동일 · ④ 복제는 캐시 공유·별도 컨텍스트는 아님.
**판별력:** `main` 이 캐시 없는 `import()` 를 부르게 되돌린 변형은 ①②가 FAIL(`constants` 2·3회, 로그 `s5_mutant_uncached`), 수정본은 PASS(`s5_cache`). 변형은 검사 후 원복했다(원본 사본은 후보 밖).
**검사:** `fmt --check` · `clippy --all-targets -D warnings` · `build --release --bins` PASS · lib `conformance*` 15 · bench bin 4 · API 10 · bench 하네스 6 · CLI 12(교정 4 포함 그대로) · helper 12 · import cache 4 PASS(`s5_targets2`) · 동결 CLI 7/7 + SystemExit 사례 재생 불일치 0(`frozen_replay_import_cache.log`).
공유 모듈(`captured_process`)은 이번에 바뀌지 않았다 — 공유 회귀 재실행 없음. 전체 Cargo·반복 루프·원본 재실행 없음. 계측 계약·ID·경계·Root 동결 파일 무변경.


### R14 외부 수용·배포·성능 범위

검사기 자체의 현행 248줄을 이행했다(초기 원형 184줄과 구분). 기본 실행 41개 검사·실제 reporting/publication/lite-metrics 런처 경계를 유지하며 원본 Python 파일은 참조로 남긴다. D1 dict-detail의 부분 출력 뒤 TypeError, reporting 프로토콜 전체 파싱, 명시된 SystemExit 문자열 사유의 비-traceback 종료와 성공 import 상태도 보존한다. 성공 LiteBindings만 같은 Context/클론에서 재사용하며 실패 import·측정·파일·판정·보고서·출력은 캐시하지 않는다. 벤치 관측기의 ‘캐시 없음’은 기대 결과를 생산 연산에 주거나 결과를 재사용하지 않는다는 뜻이며 이 생산 import 상태는 존재한다.

외부 품질은 적용 조건이 유지되는 결과를 결합했다: 현재 Cargo 사례 448(변경 무관 385 재사용 + 관련 63), 별도 FD 사례 22, fmt·Clippy·명시 release 기동 검사 16, 실제 설치 런처 API 34/CLI 7 및 설치/부재 5. 첫 전체 Cargo의 설치위치 음성검사 전제 오류는 그대로 기록하고 같은 exit 2 기준을 설치 구조 밖 실행파일에서 검사하도록 고쳤다. 새 공백 경로에는 Git index 소스 647개(이 패키지 573개와 필요한 의존 자산 74개)만 전달해 offline build와 관련 178개 현재 사례·파일 일치를 확인했다. 전체 배포 검사 131개는 한 번 통과했다. 이후 성공 import 캐시 교정은 첫 cold CLI의 호출·값을 바꾸지 않았고 영향 검사·새 소스 기동을 갱신했다. 물리적인 다른 PC·서브·다른 CPU 아키텍처에서 실행한 결과는 아니다.

배포용 산출물은 Cargo 검사를 끝낸 뒤 명시 build를 다시 수행한 기본 feature의 실행파일이다. Cargo test가 dev-dependency feature를 합쳐 같은 이름의 실행파일을 재링크할 수 있으므로 검사 빌드의 hash를 배포 빌드의 hash로 주장하지 않는다.

동일 고정 입력의 11 교차 round 결과(현재 aarch64 Linux GNU, CPython 3.12.3, 관리 Rust 1.99.1):

| 경계 | Python | Rust | Python/Rust |
|---|---:|---:|---:|
| 출력 함수 11개 × 1000, 실제 출력 생성·메모리 capture·반환 | 1.309618 μs | 0.734948 μs | 1.781918 |
| 실제 파일 읽기 10개 × 100, bind/read/decode/전체 반환 | 5.170680 μs | 2.598836 μs | 1.989614 |
| 정상 전체 검사기 1개 × 1, 실제 자식 프로세스·IO·전체 출력 | 71.294693 ms | 81.548518 ms | 0.874261 |
| 실제 Bash CLI 3개, round 평균의 중앙값 | 73.540385 ms | 52.894029 ms | 1.390334 |

전체 검사 함수는 여전히 Rust가 약 14.4% 느리다. 이를 다른 경계의 향상으로 대체하거나 모든 Rust 연산·LLM·서빙의 개선으로 표시하지 않는다. 후속 대표 Rust 시간 평가에서 이 잔여를 유지한다. 처음 측정한 더 느린 결과와 import 호출 1대2 차이·교정 기록도 보존했다.

최초 함수 66/CLI 72 원시를 보존한 뒤, Python의 silent read에만 잘못 들어갔던 stdout capture 관측 비용을 교정했다. 같은 10개·100회·11 round의 read 22개만 새로 측정하고 유효한 함수 44/CLI 72, 총 116개 기록을 재사용했다. 최종 분모는 함수 66/CLI 72다. read 비율 변화 전부를 capture 제거의 인과효과로 주장하지 않는다. 전체 값·stdout 비교는 warm에서 강제되고 timed 구간은 예외를 거부한다. 매 timed 반환값까지 전부 대조했다고 주장하지 않는다. aftertree 범위는 해당 격리 root의 type/파일 bytes/symlink target이며 전역 파일시스템이나 모든 metadata의 증명이 아니다.

범위는 현행 배포의 UTF-8 문자열 경로와 message:string 예외 프로토콜이다. 서로 다른 Context의 Python 전역 module-name identity, 모든 Python 객체·예외 생성 인자/속성·소스 traceback frame의 보편 동등성은 주장하지 않는다. KeyError의 풍부한 literal 객체 복원도 현재 문자열 키 소유자 경로 밖 한계다. 정적 배선 검사는 후속 소유자 이행 시 실제 Rust 소유 경로에 맞춰 갱신해야 한다. 전체 프로젝트의 Python 잔존 0이나 S1–S7·하네스·6프로필·실제 Long E2E 완료가 아니다.


## R15 프리셋·탐색 검증기 이행 — 후보 기록 (정본 적용 전)

상태: 이 변환 단위의 후보 구현과 아래 자체검사 COMPLETE. 정본 연결·고정 8사례 재수행·독립 검증·성능·배포 검사는 이 후보의 범위 밖이며 NOT_RUN이다. 전체 과업(S1–S7·하네스·6프로필·실제 Long E2E)의 완료가 아니다.

범위는 현행 검증 프로그램 진입점 3개(`selftest_bench_preset.py` 292줄 · `selftest_broad_search_cell_args.py` 236줄 · `selftest_broad_search_precheck.py` 362줄)다. 원본 Python 파일은 대조 기준으로 남는다.

| 원본 | Rust | 비고 |
|---|---|---|
| `selftest_bench_preset.py` `run()` | 기존 `bench_preset_selftest::run_self_test` (R6) | **다시 이행하지 않았다.** 정상 stdout 4777바이트가 원본과 같다. 교정 1건: 스위트를 끝내는 `PresetError`는 원본이 `bench_preset.PresetError`로 보이므로 그 한 줄만 모듈 한정(`uncaught_line`) |
| `selftest_broad_search_cell_args.py` `main`·`_build`·`_init`·`_cell`·`_state` | `broad_search_cell_args_selftest` (`main`·`build_checkout`·`init_sweep`·`run_cell`·`run_cases`) | N1~N6, 배포 `broad_search.sh`와 선언된 복사 목록 33개 |
| `selftest_broad_search_precheck.py` `main`·`_build_fixture`·`_lockset`·`_record` | `broad_search_precheck_selftest` | A~M, 복사 목록 33개(원본과 같은 순서) |
| 두 프로그램이 공유하는 `tempfile`·`subprocess`·`shutil`·`shutil.which`·`json` 사용 | `verify_fixture` | 격리 저장소·임시 디렉터리·자식 프로세스·검사 보고 |
| 두 `__main__` | `broad_search_selftest_cli::{run_cell_args, run_precheck}`, bin `selftest_broad_search_cell_args`·`selftest_broad_search_precheck` | 명령줄 인자는 원본처럼 읽지 않는다 |

**보존한 계약.** 실제 배포 `broad_search.sh`와 그 의존 파일을 격리 git 저장소에 같은 상대경로로 **바이트 그대로 복사해** 실행한다(재구현 없음). 부하 스크립트(`sweep_bench.sh`·`judge_bench.sh`)는 cell_args에서 argv만 기록하고 환경이 정한 종료 코드로 끝나는 stub이며 측정 산출물을 만들지 않는다. precheck에는 복사하지 않는다. `curl`은 PATH 앞단 shim(cell_args 200 · precheck 000)이다. 분류기·반복 축·프리셋·보고 체인은 복사한 실제 Rust 실행파일과 Bash 런처로 실행하고, 캠페인 소유자·템플릿 검증기는 명시된 외부 Python 의존으로 복사해 호출한다(소유자 정책을 복제하지 않았다).
- 출력: 검사 줄 `  PASS …`/`  FAIL …`은 stdout, 요약은 cell_args가 stdout · precheck가 stderr. 종료 0/1. 실패 상세는 자식 stderr의 **마지막 400/300/600 문자**(바이트가 아니라 문자).
- `git` 부재는 이름 있는 실패 한 줄(stderr, 종료 1)이며 아무것도 만들지 않는다. 격리 저장소 `git init`은 자식이 stdout/stderr를 공유하는 `check=True`·60초 호출이고, 실패는 `subprocess.CalledProcessError`(종료 코드 또는 `died with <Signals.…>`), 초과는 `subprocess.TimeoutExpired`다. git이 이미 쓴 출력은 그대로 남는다.
- `init` 거부는 `SystemExit(문자열)`: 자식 stderr 전체와 개행을 stderr에 쓰고 종료 1(트레이스백 없음).
- 복사할 배포 파일이 없으면 `FileNotFoundError`(원본 경로)로 끝난다. 잠금 템플릿이 없거나 깨져도(`json.decoder.JSONDecodeError`) skip·성공으로 접지 않는다.
- `has_yaml`: 실제 인터프리터에 `import yaml`을 묻는다. 있으면 gmu 불일치 기재를, 없으면 '대조 불가' 기재를 검사한다. 인터프리터가 PATH에 없으면 `python3`를 그대로 기동해 그 실패(`FileNotFoundError`)가 올라온다(원본은 실행 중인 인터프리터로 대체했으나 이 이식에는 그런 인터프리터가 없다).
- 정리: 정상 경로는 디렉터리 삭제 실패를 `OSError`로 올리고, 오류 경로는 `Drop`이 최선으로 지운다(파이썬의 권한 복구 재시도는 이식하지 않았다).
- 환경: 자식은 부모 환경을 그대로 상속한다(`PATH` 앞단 shim만 추가). 부모의 `*_BIN`·`*_REPO_ROOT` override를 몰래 끄지 않는다. 검사는 환경을 비우고 `PATH`·`TMPDIR`·저장소만 지정한다.

**이 이식의 것(원본에 없다).**
- 저장소 위치: 원본은 자기 파일 위치에서 저장소를 구했다. 이 이식은 명시 환경 `BENCH_VERIFY_REPO_ROOT`가 우선이고 없으면 실행파일의 설치 배치(`…/adversarial-benchmark/rust/target/release/<이름>`)에서만 읽는다. 브랜치·에이전트 카드·토폴로지로 추정하지 않는다. 둘 다 없으면 이름 있는 한 줄과 **종료 2**이며 아무것도 만들지 않는다(`git` 확인이 먼저다).
- 임시 루트는 Linux GNU의 `tempfile` 후보 순서(`TMPDIR`·`TEMP`·`TMP`의 비어 있지 않은 값, 고정 시스템 루트 `/tmp`·`/var/tmp`·`/usr/tmp`, 마지막으로 현재 디렉터리)에서 쓸 수 있는 첫 곳이다(R15 교정 2에서 정정 — 아래). 디렉터리 이름의 8자 접미는 `ring`의 시스템 난수를 균등하게 거부 표본한 것이다.
- 신호 이름표는 Linux 1~31이다. 실시간 신호는 `died with unknown signal N.`이다.

**등록 검사(`checks/broad_search_selftest_cases.rs`, 16 + 라이브러리 단위 11).** 모든 시나리오가 실제 바이너리를 실행한다: 배포 파일 두 통과 실행(검사 줄 14/27개·요약·빈 stderr·임시 디렉터리 제거), 인자 무시, **변이** 3(cell의 `--authority` 거부 제거 → N4만 FAIL 13/1, precheck의 거부 분기 제거 → 분류 검사 2개 FAIL 25/2, 판정 분기 둘 제거 → A~D FAIL), `git` 부재 2, 알 수 없는 저장소 2(미설정·빈 값), 설치 배치로만 저장소를 아는 실행, 출력하고 실패하는 git·신호로 죽는 git, 파일 누락 2, `init` 거부 2, 깨진·없는 템플릿, PyYAML 탐지 두 갈래. 실패 시나리오마다 임시 디렉터리 제거를 확인한다. 라이브러리 단위는 문자 단위 자르기·템플릿 id 부여·`dict` 갱신 오류·멤버십·키 제거·임시 디렉터리·`copy2` 오류 귀속·`exists`·`unlink`·신호 이름이다. 프리셋은 `checks/preset_selftest.rs`에 변이 대조 1건을 더했다(교정 전 표기에서 FAIL, 교정 후 PASS를 확인).
**검사 결과(관리 Rust 1.99.1, offline·locked):** `fmt --check` · `clippy --all-targets -D warnings` · `build --release --bins` PASS · 전체 `cargo test` 116 결과 블록 475 PASS/0 FAIL/1 ignored(기존 GuideLLM CA 외부 통제 환경이 없을 때 건너뛰는 검사) · 이후 영향 범위 재실행 라이브러리 144 · 신규 16 · package_paths 2 · preset_selftest 4 PASS. 정상 실행의 stdout은 원본 고정 출력과 바이트가 같고(15줄 · 28줄 · 프리셋 4777바이트, 각각 요약 줄 포함) stderr는 비며 종료 0이다. 고정 실패 사례 중 변이 2 · git 부재 2 · 프리셋 변이 1도 원본 기대와 일치했다(임시 루트 이름만 정규화).
**NOT_RUN:** 독립 검증자의 별도 원본 2사례 · 정본 설치(래퍼·build 18기동·배포검사) · 성능 비교 · 물리적으로 다른 노드·아키텍처 · 기존 CA 외부 통제 검사.

### R15 교정 2 — 임시 루트 후보 순서 (후보 · 정본 적용 전)

**초판(v1)의 결함.** v1은 `TMPDIR`·`TEMP`·`TMP` 다음에 `std::env::temp_dir()`를 "플랫폼 기본 루트"로 넣었다. 그 함수는 **사용할 수 없는 `TMPDIR`를 그대로 돌려주므로**, 존재하지 않는 중첩 경로의 `TMPDIR`와 미설정 `TEMP`/`TMP`에서 고정 시스템 루트에 닿지 못하고 현재 디렉터리가 선택되었다. 원본은 `/tmp`를 고른다. 초판 기록의 "고정 시스템 루트 둘은 기본 루트가 쓸 수 없는 호스트에서만 차이" 한계 서술은 동등성이 아니었으므로 **철회**한다. 실제 결함 증거(원본 `r15-tempdir-reference`, v1 후보 `r15-tempdir-candidate`)와 v1 소스·실행파일 보관본은 그대로 보존한다.

**수정.** 후보 목록을 `temporary_candidates`(환경 조회를 인자로 받는 순수 함수)가 만들고, `first_usable_temporary_root`가 `.`이 아닌 후보를 절대 경로로 바꿔 프로브 파일을 만들고 지울 수 있는 첫 곳을 고른다. 고정 시스템 루트는 `SYSTEM_TEMPORARY_ROOTS`(`/tmp`·`/var/tmp`·`/usr/tmp`)이며 정당한 OS 상수다. 프로세스 전역 환경은 바꾸지 않는다. 모두 쓸 수 없으면 `FileNotFoundError: [Errno 2] No usable temporary directory found in [후보 목록]`이다. 범위는 Linux GNU이며 다른 OS의 후보는 이식하지 않았다.

**등록 검사.** `broad_search_selftest_cases`의 `an_unusable_temporary_directory_variable_falls_through_to_the_system_root_before_the_working_directory`: 실제 두 바이너리를 존재하지 않는 중첩 `TMPDIR`·미설정 `TEMP`/`TMP`로 기동하고, 종료 17로 실패하는 가짜 `git`이 보고한 저장소 경로의 부모가 `/tmp`이며 이름이 `bs-cellargs.`/`bs-precheck.`로 시작하고, 종료 뒤 그 디렉터리가 없고 작업 디렉터리가 비어 있음을 확인한다. **v1 소스에서 FAIL**(부모가 작업 디렉터리, 원시 로그 `v2-regression-on-v1`)하고 교정 후 PASS(`v2-regression-fixed`)했다. 라이브러리 단위 2개는 후보 순서(빈 값 건너뜀·고정 루트·현재 디렉터리 또는 `.`)와 쓸 수 없는 후보 건너뛰기·오류 목록을 환경 변경 없이 확인한다.

**유지되는 한계(이번에 바꾸지 않았다).** 실시간 신호는 `died with unknown signal N.`이며 신호 이름표는 Linux 1~31이다. 인터프리터가 PATH에 없으면 `has_yaml` 탐지는 `python3`를 그대로 기동해 `FileNotFoundError`가 된다. 오류 경로의 임시 디렉터리 정리는 최선(`Drop`)이며 파이썬의 권한 복구 재시도는 없다.


## R15 controller integration and measured whole programs

The actual standalone entries are `selftest_bench_preset.sh`, `selftest_broad_search_cell_args.sh`, and
`selftest_broad_search_precheck.sh`. The first reuses the existing full preset suite; the two new programs copy and
exercise the deployed Broad Search shell script. Distribution checks and the two policy call edges use the new
launchers. The build runs 18 startup checks. The originals remain reference programs.

The controller compared the eight frozen CLI cases, an additional original-first unusable-TMPDIR case, and nine
copied-installation cases. The unusable-TMPDIR regression failed on v1 and passed on v2. The preset observation first
bypassed the preexisting owner-adapter binding; using the actual preset launcher corrected that observation without
changing its expected result. Independent validation compared two separately frozen failures once each; the first
comparator retained the native frame placeholder, then reprojected the saved streams under the existing source-frame
rule. Qualified exception messages, their multiline payload, pretrace child diagnostics and partial stdout remain
compared. Valid explicit-TMPDIR results were reused across the v2 candidate-selection-only production delta.

Canonical validation passed fmt, 36 affected release checks (13 fixture units, 17 program cases, four preset checks,
two package-path checks), Clippy, and the default product build followed by 18 startup checks. The current logical
479 passing Cargo cases comprise 443 unchanged cases from the preceding unit plus these 36; this is not a new
single full 479-case invocation. The author's earlier full run passed 476 cases, with one preexisting CA-control
case ignored. That ignored case is not counted as passed. The separately exercised 22 descriptor scenarios are
reused because their shared process-capture source is unchanged.

Fresh Git-index-only installation passed with 683 files (580 Rust-package files plus 103 declared dependency files),
without copying build targets, credentials or caches. It reused the node-managed toolchain cache explicitly, built
offline, ran the 18 startup checks and 36 affected release checks, then restored default-feature product binaries.
The whole distribution suite passed 131/131 in one invocation (250.618041 seconds). This is a fresh local-path
installation, not a physical second-PC or subnode run.

Timing used three actual whole programs, one warm pair and five alternating measured pairs (36 CLI records). Fixture
preparation by the observer was outside the timer; the programs' private fixtures, Git initialization, actual owner
processes, IO, report output and cleanup were inside. Every stdout/stderr/exit matched the frozen original and the
observed private source closure stayed unchanged. No artificial pure fragment was timed.

| Whole CLI | Python median seconds | Rust median seconds | Python/Rust |
| --- | ---: | ---: | ---: |
| preset verifier | 4.675954906 | 3.873460354 | 1.207177686 |
| cell-args verifier | 1.818915417 | 1.840113415 | 0.988480059 |
| provenance-precheck verifier | 6.346408067 | 6.348014563 | 0.999746929 |

The median of each side's three-program round means was 4.273165523 versus 4.033247599 seconds (1.059485047).
This aggregate is not the mean of the three speedup ratios. Both Broad Search boundaries show no measured speedup;
the small differences do not establish statistical significance. They remain in the final Rust-time review together
with the preceding timing limits. No LLM, GPU, serving, six-profile evaluation, or whole-project completion follows
from this unit. Python campaign owners and embedded production snippets remain explicit later-stage dependencies.

Evidence directories: `r15-canonical-checks/`, `r15-packaging/`, `r15-distribution/`, `r15-benchmark-results/`,
`r15-tempdir-correction/`, `r15-installed-candidate/`, and the independent `candidate-v1/validation-delta/`.

## R16 판정·lite·메타 검증기 이행 — 후보 기록 (정본 적용 전)

Status: **INCOMPLETE for the whole task** (this unit's Rust scope is complete only as far as the checks below say; Root
integration, installed-copy acceptance, the 48-record performance run and the six-profile/LongE2E stages are not this
unit's). Source head `7e2aea9d8c18f8cacd99783a406e2961e03fb9b8`; the candidate starts from the 580 Git-index Rust
sources of that head.

### Scope and output

Four deployed verification programs, 1432 source lines, are ported as whole programs, not as golden PASS prints:

| Original (`.claude/skills/adversarial-benchmark/scripts/`) | Lines | Rust program | Rust module |
| --- | ---: | --- | --- |
| `selftest_judge_bench.py` | 370 | `selftest_judge_bench` | `src/selftest_judge_bench.rs` |
| `selftest_lite_report.py` | 392 | `selftest_lite_report` | `src/selftest_lite_report.rs` |
| `selftest_lite_gate.py` | 226 | `selftest_lite_gate` | `src/selftest_lite_gate.rs` |
| `selftest_sweep_meta.py` | 444 | `selftest_sweep_meta` | `src/selftest_sweep_meta.rs` |

Shared pieces: `src/bench_verify_support.rs` (the printed `[PASS]`/`[FAIL]` list in the two detail styles and the few Python
evaluations the conditions are written in), `src/bench_verifier_cli.rs` (command lines and exit mapping),
`src/owner_bridge.rs` + `src/owner_bridge.py` (the transitional Python-owner transport, below), four `src/bin/` entries,
`checks/bench_verifier_cases.rs` (ten registered program checks). The one change to an existing module is
`verify_fixture::run_text_within` (the original `run_text` now delegates to it with its unchanged 120 seconds; the judge
program calls use 180 and the full-report render 60, as the originals did).

Structure that follows the originals: `Sandbox` of the lite report program is the one the lite gate program runs in
(the original `selftest_lite_gate.py` imported `selftest_lite_report.Sandbox`); `SweepMeta::build_meta` of the meta
program is the one the lite report program's L8 uses (the original imported `selftest_sweep_meta.build_meta`). Case
tables (C1–C10, S1–S10, RC1–RC3, W1–W7, J1–J12, K1–K9, L1–L10) keep the originals' names and printed text.

### Native pieces reused, Python pieces transported

Reused typed Rust libraries instead of the original's Python transport modules: `render_report::{lite_identity,
Environment, MEASUREMENT_CONFIG_TITLE, MEASUREMENT_CONFIG_KEYS, LITE_BENCH_TOOL}`, `bench_naming::{bench_filename,
measurement_ts}`, `classify_cell::{bench_mode_kind, BENCH_MODE_*}`, `lite_metrics::read_lite_verdict` and the verdict exit
table, `guidellm::inherit_accept_len` (the W1 negative control). The original loaded `render_report_native.py`,
`classify_cell_native.py`, `doc_naming_native.py`, `lite_metrics_native.py` and `parse_guidellm_native.py`; this port does
not load them, so these programs no longer exercise those transports (their other consumers are unchanged).

Current Python dependencies, each a real current owner and none copied or answered locally:

| Owner | Reached by | Operations (fixed list) | Until |
| --- | --- | --- | --- |
| hint reader `hint-publisher/scripts/render_bench_section.py` | owner bridge | parse_measurement_config, is_lite_report, lite_metrics_present, render_lite(parse_lite_report), parse_report level count, four contract constants | S6/S7 port of the reader |
| evidence publisher `policies/runtime/evidence_publisher.py` | owner bridge | bench-report name pattern match, `report_measurement_config(text)[0]`, `LITE_BENCH_MODE` | S6/S7 |
| completion gate `policies/runtime/completion_gate.py` | owner bridge | `parse_flat_certificate`, `certificate_run_key(fields) is not None` | S6/S7 |
| production Python embedded in the deployed `sweep_bench.sh` (`python3 - <<'PY'`) | owner bridge (`run_embedded_program`) | the extracted source runs per fixture as `exec(compile(...))` | S1 embedded-code migration |
| wiki TS document naming (reached by the publisher's `doc_naming.py` shim) | the publisher | not called by this port directly | TS deferred |

The deployed `sweep_bench.sh` text, its embedded program and the shell function `_spec_axis_args` are read from the
file and executed as deployed; no parser is cloned. The original verifier programs are never run and nothing falls back
to them.

**Owner bridge** (`owner_bridge.py`, compiled into the crate with `include_str!` and passed as `python3 -c`; no file is
deployed and no interpreter is installed): one persistent interpreter for the whole run, requests one JSON line, replies
one JSON line on a private copy of standard output; what an operation prints is captured per request and replayed in
order on the verifier's streams; exceptions return their class, `str()` and `repr()` (a `SystemExit` its reason or
status). There is no evaluation of caller-supplied code or callable names. Owner modules are loaded as the original
`_load` did (`sys.path.insert(0, parent)` then `exec_module`, same module names), the first path entry is the deployed
scripts directory as when a script is run, and nothing is cached: each owner call is a call (the evidence publisher
imports the reader again at every call, so a reader that appends a marker on import receives five lines in the lite
report program: one for the program's own binding and one for each of four publisher calls, as the original). The
interpreter is `python3` on `PATH` or `OWNER_BRIDGE_PYTHON`; an unusable one is a named `FileNotFoundError` traceback
before any check. The session sets `PYTHONDONTWRITEBYTECODE=1` (the original wrote `__pycache__` into the deployed tree).

### Preserved contracts

Default calls keep the originals' meaning: no new public flags. The judge and lite gate programs ignore all arguments;
the lite report and sweep meta programs look only for `-v`. Context: `BENCH_VERIFY_REPO_ROOT`, else the installation
around the running binary (`<repo>/.claude/skills/adversarial-benchmark/rust/target/release/`); an unknown context is
named with exit 2 before any fixture; the lite programs name a missing `git` first (exit 1). Check lines, closing
lines, failure summaries (`[..] FAIL n 건: [...]`, the meta program's `  - ...` list), the judge's `name detail` and the
lite programs' `name — detail[-900:]` styles, partial output before an exception, `SystemExit` with a string reason
(stderr + exit 1), cleanup of every `TemporaryDirectory` including after failures, child timeouts (judge 180/120, lite
120, full render 60, shell function 60) are kept. Embedded-program stdout/stderr (23 `[sweep_bench] sweep_index.json →`
lines, two warnings) reach the verifier's streams unchanged. The sandbox files are written as the originals wrote them
(text copies of scripts through UTF-8 read/write, byte copies of release programs with permission bits).

### Evidence (builder self-checks; actual canonical tree read-only, `BENCH_VERIFY_REPO_ROOT` explicit)

* Default four, candidate debug binaries against the frozen original-first `normal-cases.json`: stdout/stderr identical
  for all four after normalizing only the declared dynamic temporary roots (`probes/n2-*`).
* Failure/effect nine against private copies of the frozen nodes: identical exit, stdout and stderr for all nine except
  one tail-cut random temporary-name fragment inside a 900-character detail window (`probes/f2-*`,
  `probes/normalize_check.py`); the import-effect case leaves exactly five `rbs-import` lines and exit 0.
* Verbose two: the meta program identical; the lite report program identical except the one live `/proc` RAM table cell
  (observation variance, form checked) (`probes/v1-*`).
* Commands (working directory = the candidate crate, node-managed Rust 1.99.1 through the project bootstrap, `--offline
  --locked`, external target directory, `BENCH_VERIFY_REPO_ROOT` and the owner-input variables explicit): `cargo fmt --
  --check` PASS; `cargo clippy --lib --bins -- -D warnings` PASS; whole `cargo test` (debug) exit 0 with **489 passed, 1
  ignored** (the one ignored case is the preexisting CA-control case, not counted as passed; 479 earlier logical cases + the
  10 new registered cases) over 121 result lines; the new `checks/bench_verifier_cases.rs` 10 PASS, the impacted
  `broad_search_selftest_cases` 17 PASS and `package_paths` 2 PASS (shared `verify_fixture` change). After the full run only
  a doc-comment placement in `selftest_sweep_meta.rs` changed; fmt, clippy, the 10 cases and package paths were rerun PASS.
  The final explicit default `cargo build --release --bins` (after the tests) exit 0; the four new binaries are 1.19–1.53 MB.
  Logs: `logs/test-full1.log`, `logs/test-bv4.log`, `logs/fmt-check3.log`, `logs/clippy3.log`, `logs/build-release1.log`.
* The **release** binaries repeat the frozen comparison: normal four identical (`probes/rel1-*`); failure/effect nine
  identical except the same tail-cut random temporary-name fragment (`probes/rf1-*`).
* The canonical Git tree stayed clean at `7e2aea9` and no bytecode cache was created in it by these runs.
* The package-path guard holds: no `test`/`tests`/`temp`/`tmp` path component and none of the forbidden content literals in
  the new files.

### Minimum coupling Root needs (proposed, not done here)

Launchers for the four programs with the wrapper variables `SELFTEST_JUDGE_BENCH_BIN`, `SELFTEST_LITE_REPORT_BIN`,
`SELFTEST_LITE_GATE_BIN`, `SELFTEST_SWEEP_META_BIN`; the `judge_bench.sh --self-test` dispatch to the new judge launcher; the
build check list 18 → 22 programs (the four new names); the existing distribution check replacement (its 131 stay), call
edges and the copy closure: each program reads the deployed files the original read (the benchmark scripts and release
programs the sandboxes copy, `sweep_bench.sh`, the hint reader, the evidence publisher, the completion gate with its schema
and wiki naming dependencies, `campaign_init.py` and its validator, `campaigns/_template`). The owner bridge adds **no
deployed file** (compiled in) and needs `python3` on `PATH` (or `OWNER_BRIDGE_PYTHON`). Without `BENCH_VERIFY_REPO_ROOT` the
programs find the repository only from an installation under `rust/target/release`.

### R16 v2 corrections (after the independent and Root source-first findings)

Fixed, each with a registered regression in `checks/bench_verifier_cases.rs` and the frozen v1 failures kept in the Root
and independent records:

* **I1, qualified owner class.** The session sends `module.qualname` (bare for `builtins` and `__main__`) with every
  exception; `Raised::with_qualified_class` carries it and the shared `traceback_line` prints it. `failure().class` stays the bare
  `__name__` (the meta program's `type(exc).__name__` text). Generic, no hard-coded class. `SystemExit` keeps its status/string.
* **Raw and partial output, buffering order, standard input (v3).** The session's channel is a socket pair left open
  across the spawn under a descriptor number given as an argument; descriptors 0, 1 and 2 are the verifier's own, so raw
  `os.write`, inheriting children and reads of the real standard input behave as in the original process. The session's
  `sys.stdout`/`sys.stderr` send every `write` and `flush` as an event and wait for the verifier's acknowledgement, so an
  owner cannot do its next raw write before the event is applied. Stdout events go to `Console::write`/`flush`, i.e. the
  verifier's own Python-faithful buffer (block buffer; line flush on a terminal; write-through when `PYTHONUNBUFFERED` is
  non-empty, new `StdoutProfile::unbuffered`), so owner text and earlier check lines share one buffer as in one process;
  stderr events are written at once. Replies are read to completion (300000-character diagnostics checked). A session
  that is gone (reset, closed, exited before the handshake) is a named `OwnerBridgeError`. Root's raw-output, order and
  the three stdio edge references (stderr order, unbuffered order, inherited stdin) give the original's exact stdout and
  exit status; only the Python source-frame lines of the traceback differ (native placeholder).
* **Eager details.** `BracketChecks::check` evaluates the detail right after the condition for a passing check too;
  an exception while building it propagates (`and_then_value` and the detail closures return `Result`).
* **Python `\d` and `strip`.** The measurement-time shape uses the project's Unicode decimal-digit predicate;
  `strip`/`lstrip` use Python's white space set (`py_strip`, `py_lstrip`). Unit-tested (Arabic-Indic digits, U+001C).
* **Same-file copy.** `verify_fixture::copy2` refuses a destination that is the source's own inode
  (`shutil.SameFileError`, as `shutil.copy2`), and the check fixtures copy through `copy_detached` (unlink the name,
  copy as a new file, assert a new inode).

Remaining differences (none found by a frozen comparison): stderr text without a line break and without a flush is held
by a line-buffered stream until a later newline, flush or exit in the original, here until the end of the operation
that wrote it; `Path.glob` tie order; the Python owners and embedded program stay current dependencies (not Python-zero);
no timing is claimed.

Final v3 checks (current source, unchanged since): `fmt --check`, `clippy --all-targets -- -D warnings`, and one run of the
affected set `logs/n-t5.log` (lib 153, `bench_verifier_cases` 20 including stderr order, unbuffered order, inherited standard
input of 12 bytes `owner-stdin\n`, flush order, early-exit session, the shared-inode copy regression and the bounded
start-only `ETXTBSY` retry of the check runner; Broad 17, `package_paths` 2, stdout boundary 7, descriptors 10, conformance
12/12/4/4, dataset 1/1/2, reporting 4/1, `python_semantics` 7, `cli_contract` 6; all PASS). The three Root stdio edge
originals give identical stdout and exit status (`probes/s1-*`, stderr identical but for the Python source frames); the
earlier 17 comparisons repeat as before (`probes/c4-*`). No `unsafe` and no `pre_exec` is used: the channel is a safe
`UnixStream` pair whose session end has its close-on-exec flag cleared with `rustix::io::fcntl_setfd` and is named by
number; the verifier closes its copy after the spawn and the session marks it non-inheritable for its own children. No
network, TCP or permission workaround. The old v1 stdin control was actually rerun during the stdio-fix attempt and hung for more than 584 seconds; the Bash tool ended it at its 600-second timeout. The outer attempt later timed out. The Root sent no signal. This corrects the author handoff claim; the original logs and failure are retained.

### Deviation record: canonical runtime programs truncated by a check fixture (truthful)

The first v1 check copied the deployed release programs into a private tree by hard link, and its second call in the same
scratch directory copied each file onto itself with `std::fs::copy`, which truncates the shared inode. This zeroed the 43
canonical generated release programs (and their same-inode dependency aliases) during the v2 work. It was an indirect write
outside the allowed output directories. The tracked source stayed clean at `7e2aea9`. The Root restored all 43 programs
atomically from the frozen base copies (`r16-runtime-recovery/`); the builder wrote nothing to the canonical tree. The
fixture no longer links or copies onto an existing name (`copy_detached`), and `copy2` refuses a same-inode destination.
Evidence: `probes/canonical-release-truncation.json` (the damage inventory, taken before the Root restoration) and
`probes/canonical-proof-{before2,after2,...,after5}.json`: the 43 canonical program inodes, sizes and SHA-256 and the
frozen base's 43 are unchanged across every v2 command. Forty-five generated cache files in the canonical release directory
(44 `*.d`, one `.rlib`) are still zero bytes (the three zero cargo lock files are normal); they are build metadata, not
run by any check, and are for the Root to regenerate by its own scoped build. This is not a claim that every path is protected.

### Limits and deliberate boundaries (still true)

* Python tie order of `Path.glob` for two raw files with an identical modification time is not reproduced.
* The bridge, the embedded-program source and the three Python owners are current dependencies, not removed ones;
  stage S1 (embedded code) and S6/S7 (reader, publisher, gate) replace them. This is not a Python-zero result.
* Performance is not claimed: the builder ran no timing; Root's four-program 48-record contract and the earlier Rust-time
  limits (including R15 BROAD2 0.988480/0.999747) stay for final review.


## R16 controller integration and measured whole programs

Four native verification entries now run through `selftest_judge_bench.sh`, `selftest_lite_report.sh`,
`selftest_lite_gate.sh` and `selftest_sweep_meta.sh`. The actual judge `--self-test` dispatch and three distribution
calls use those launchers. The explicit build runs 22 startup checks. Rust owns the verification checks and fixtures;
the deployed Python hint reader, publisher, gate and embedded sweep program remain explicit transitional dependencies.
There is no fallback to the original Python verifier programs. These originals remain references.

Root compared 20 original-first CLI cases and 17 copied-installation cases. Independent validation compared two
separate original-first faults, then reused the valid actual results across reviewed source changes. It directly
audited the five saved new stdio cases without repeating the originals. The initial namespace, raw-output and
stdio failures, observation corrections, budget checkpoints, capacity recovery and fixture damage are retained.

Canonical fmt, all-target Clippy, 192 selected release cases (153 library, 20 verifier, 17 Broad Search, two package),
and the default product build with 22 startup checks passed. This is not a claim of a new full Cargo run. Fresh
Git-index-only installation passed with 815 files (593 Rust-package files), the same 192 selected cases and 22
startup checks, followed by explicit default-feature product restoration. No build target or authentication files
were copied. The node-managed toolchain and declared cache were explicitly reused. Whole distribution passed
131/131 in one invocation (250.554595 seconds). A second physical PC, subnode and x86 execution were not performed.

The damaged generated package cache was repaired with a scoped `cargo clean --package sweep-stop --release` and
managed offline locked `build --release --lib --bins`. All 47 product ELF files and 49 root metadata/library files
are nonempty, and 22 actual startup checks passed. Global toolchain, registry and unrelated package caches were not
cleaned. The prior 43-program atomic recovery and zero-cache inventory remain historical evidence.

Timing used four actual whole programs, one warm pair and five alternating measured pairs: 48 new records,
eight warm and 40 measured. Observer clone preparation was outside the timer; the programs' fixtures, Git, deployed
owners and embedded program, IO, full output and cleanup were inside. Every whole stdout/stderr/exit matched the
frozen reference under the declared fixture-root projection, and the observed 256-file closure stayed unchanged.

| Whole CLI | Python median seconds | Rust median seconds | Python/Rust |
| --- | ---: | ---: | ---: |
| selftest_judge_bench | 1.374262545 | 1.398833672 | 0.982434561 |
| selftest_lite_report | 0.756951480 | 0.699287623 | 1.082460857 |
| selftest_lite_gate | 1.801981819 | 1.811687955 | 0.994642490 |
| selftest_sweep_meta | 0.394957098 | 0.433377583 | 0.911346395 |

The median of four-program round means was 1.076291066 versus 1.084062000 seconds (0.992831651).
This aggregate is not the mean of speedup ratios. Report improved in this sample; judge, gate and meta showed
no measured improvement. Small differences do not establish statistical significance. These boundaries remain
in the final Rust-time review with the prior limits. No LLM, GPU, serving, six-profile evaluation or whole-project
completion is inferred. No separate pure fragment was invented for these verification programs.

Evidence: `r16-root-v3/`, `r16-owner-deltas-v3/`, `r16-installed-candidate/`, `r16-canonical-checks/`,
`r16-packaging/`, `r16-distribution/`, `r16-benchmark-results/`, `r16-runtime-recovery/`, and the independent
`stdio-delta-v3/` and `benchmark-review/`. Source-frame projections retain qualified classes, multiline messages
and preceding diagnostics. Arbitrary Python stream objects, concurrent descriptor mutation, glob ties and universal
liveness were not proved. Later stages must replace the remaining embedded and owner Python dependencies.

## R17 반복 스윕 검증기 이행 — 후보 기록 (정본 적용 전)

Status: **COMPLETE for this unit's own required checks** (fmt, all-target Clippy, the registered program checks, the
shared-path impact checks and the explicit default release build all passed) and **INCOMPLETE for the whole task**.
Root integration (the launcher, the 23rd startup check, the distribution call and edge, the SKILL guide), Root's
official whole-CLI timing, installed-copy acceptance, independent review and the six-profile/Long E2E stages are not this
unit's and are not claimed. Source head `fe4d585b5a13094f3efb0211665ff08062a8720e`; the candidate starts from the
593 Git-index Rust-package files of that head and adds seven (600 files, no `target/`, no authentication, no cache).

### Scope and output

| Original (`.claude/skills/adversarial-benchmark/scripts/`) | Lines | Rust program | Rust modules |
| --- | ---: | --- | --- |
| `selftest_sweep_repeats.py` | 988 | `selftest_sweep_repeats` (`src/bin/selftest_sweep_repeats.rs`) | `src/selftest_sweep_repeats.rs` (program, groups N P R G F V A), `src/sweep_repeats_broad.rs` (groups B O M), `src/sweep_repeats_fixture.rs` (`Sandbox`, the 67 copies, the shims), `src/sweep_repeats_clock.rs` (the M1 time arithmetic) |

Command line: `bench_verifier_cli::run_sweep_repeats` (same `run_verifier` mapping as the four R16 programs: `-v` is
looked for among otherwise ignored arguments; exit 0/1; `Class: message` after a traceback marker for an unhandled
exception, a string `SystemExit` as plain text; exit 2 is this port's "no repository known"). Library entry:
`selftest_sweep_repeats::main(repo_root, verbose, console)`. Case names, condition order, eager details and printed
text keep the original's; every one of the 75 checks (N1–N8, A3, P1–P3, R1, G×5 + conformance, F1–F8, V1, A1/A2/A4,
B1–B8, O1–O6, M1–M2 and the R★ report-row controls) is a real run of the deployed chain, not a stored result.

Reuse instead of new rules: `verify_fixture::{IsolatedCheckout (copy2 protections, git init, temporary directory with
cleanup on error), run_text_within, TemporaryDirectory, load_json, write_str, …}`, `bench_verify_support::BracketChecks`
(one new detail style, `TailedWithDashOf(700)`, because this original shows `str(detail)[-700:]` where the R16 programs
show 900), `conformance::{Raised, py_equals, splitlines, iterate_values}`, `pyvalue::PyValue` (and its Python `repr` for
details and for the `lite` dict that is part of a G check name), `pyjson::loads_text`, `owner_bridge::OwnerBridge`,
`bench_verifier_cli::run_verifier`. No generic interpreter was added. The two R16 shared files that changed:
`bench_verify_support.rs` (the added `DetailStyle` variant, one unit test) and `owner_bridge.{rs,py}` (one fixed
operation, below). Nothing of the original Python verifier is run, imported or used as a fallback.

### Preserved external behavior (what the checks execute)

* Order: `git` on `PATH` first (message to standard error, exit 1, nothing created) → repository unknown (exit 2) →
  sandbox build (copies, shims, `git init`, manifest, model, campaign) → groups in the original order
  N, P, R1, G, F, V, A, B (init, B6, cell), O, B3, B8, B4, B5, B7, M1, M2 → failures summary (`FAIL N건: [...]` on
  standard error, exit 1) or the closing PASS line.
* A deployed file that is missing is the copy's own `FileNotFoundError` with its path before any check; the temporary
  directory is removed on every path (normal, failed checks, exception, `SystemExit`).
* Limits: every sandbox call `timeout=170` s (`RUN_SECONDS`), the conformance child 120 s (`CONFORMANCE_SECONDS`), the
  checkout creation `git init` 60 s (shared). Standard input, environment and working directory of the conformance child
  are the verifier's own; the sandbox calls get the working directory = the checkout, `PATH` = shim directory first,
  `PYTHONDONTWRITEBYTECODE=1` and, when a plan is given, `SHIM_PLAN` (an empty plan object is still set).
* The product environment is inherited as the original inherited it: an owner-root override (e.g.
  `BENCH_PRESET_REPO_ROOT`) pointing at the outer tree hides the sandbox campaign and fails exactly four named checks
  (registered check). Test runners therefore clear such overrides; the program does not.
* N4: the hint reader is imported **late and once** under its own module name `render_bench_section`, with its directory
  inserted first on `sys.path` at that moment; an `Exception` of the import or `parse_report` is the failed check whose
  detail is `str(exception)`; `SystemExit` or a broken transport ends the run.
* O6: two sandbox calls run on two threads at once; exactly one wins the `mkdir` claim.
* M1: `window_end_utc` is read with `strptime("%Y-%m-%dT%H:%M:%SZ")` semantics and moved by seconds
  (`sweep_repeats_clock`: first-matching alternatives, one-digit fields, case-insensitive `T`/`Z`, Python's messages;
  unpadded year on output like glibc; confirmed against CPython 3.12 in `logs/python-strptime-probe.txt`).
* Nothing is hard-linked: every copy is a new inode through `copy2` (`SameFileError` for the same file), and the passing
  registered check proves that the deployed tree it ran on was not changed at all (contents, modes, inodes, times, no new
  file such as a byte-code cache).

### Owner bridge change (one fixed operation)

`OwnerBridge::parse_sweep_report(directory, text)` ↔ session operation `rbs_parse_sweep_report`: `sys.path.insert(0,
directory)`, `import render_bench_section`, `parse_report(text)`, and the `level` of each parsed row plus `truncated`
(and `repr(parsed)` for a failure detail). The verifier makes the comparison (`levels == [1, 2]`, `truncated == []`) and
treats a missing `truncated` as the `KeyError` the original's expression would raise. The R16 `load` role of the reader
(`_lite_rbs`) is untouched and not used here. The session is started at this first use, not at program start (the
original's import happened there), and ended right after; its failure to start is a named failure after the ten earlier
lines, not a caught check failure.

### Remaining Python and other limits (not Python-zero)

* **Shim text.** `run_bench.sh` embeds a Python program (`python3 - <<'PY'`) and `lite_bench.sh` two `python3 -c`
  one-liners; they are the original's test data, written byte for byte into the sandbox and still need `python3`.
* **Hint reader** `render_bench_section.py`: real owner, reached through the Python session of `owner_bridge`
  (interpreter `OWNER_BRIDGE_PYTHON` or `python3` on `PATH`). The session sets `PYTHONDONTWRITEBYTECODE=1`; the original
  imported in its own process and could write a byte-code cache into the deployed reader directory. This is a safer,
  deliberate difference.
* **Deployed Python-backed owners** that the chain inside the sandbox still runs (`campaign_init.py`,
  `repeat_axis_owner_query.py`, `classify_cell_role_query.py`, `doc_naming.py`, the programs embedded in
  `sweep_bench.sh`/`broad_search.sh`, …) are the product's, copied from the installed tree; their migration is later.
* Python stack-frame lines of a traceback are not reproduced (`(native: Python stack frames are not reproduced)`), as
  for R16; the qualified exception class and message are.
* `strptime` reads ASCII digits only; `Path.rglob` of the original's `tree_bytes` is followed without crossing directory
  symlinks. The wall-clock limits are constants of the shared child runner and were not exercised by a 170/120 second wait
  (NOT_RUN); the shared runner's timeout behaviour is the R16 one.
* No performance claim: Root's official 12-record whole-CLI timing is not run here. Earlier limits (R14 whole programs
  14.4 % slower, R16 judge/gate/meta without measured improvement, the OPEN S3/S4/S7/IO limits) remain.

### Original-first and frozen references

Frozen by Root before this unit (`r17-reference/`): default (75 PASS, 9061 stdout bytes), no `git`
(stderr 125 bytes, exit 1), missing deployed sweep source (stderr 1263 bytes, exit 1). The final release binary on the
private 235-file closure: default stdout **byte-identical** (`cmp`), no-git stderr **byte-identical**, missing-source
header and final exception line **identical** (middle lines are the allowed Python frames), all with an empty temporary
root afterwards (`logs/final-*`). The initial five failures (outer owner-root override and an absent conformance child)
and the first corrected run (three Rust sources and `recipe.py` read as static text by conformance) are Root's
retained observations; this program's registered check reproduces the override effect on purpose.

Three narrow original-first experiments, each run once on a detached private tree, comparing the actual original
and this program on the same mutated input (stdout, stderr, exit):

1. the hint reader raises `RuntimeError('reader boom')` on import → one FAIL with detail `reader boom`, 74 PASS, exit 1,
   summary `FAIL 1건` — identical (`logs/orig-reader-exception.*` vs `logs/probe-rb.*`);
2. the reader raises `SystemExit('reader says stop')` → ten lines, `reader says stop` on standard error, exit 1 —
   identical (`logs/orig-reader-systemexit.*` vs `logs/probe-se.*`);
3. `--help -v x` → the first sweep's output tails after the first check — identical after normalizing only the random
   temporary-directory name (`logs/orig-verbose.*` vs `logs/cand-verbose.*`).

### Checks run (managed Rust 1.99.1, offline, locked, own target; working directory = the candidate)

| Check | Result | Raw log (`logs/` of the worker's check directory) |
| --- | --- | --- |
| `cargo fmt -- --check` | PASS | `fmt-check1.log` |
| `cargo clippy --all-targets -- -D warnings` (no `#[allow]` added) | PASS | `clippy2.log` (`clippy1.log` before the last comment edits) |
| `cargo test --release --test sweep_repeats_cases` (new, 12 program checks) | **12 passed / 0 failed**, 18.89 s | `test-final-sr.log` |
| `cargo test --release --test bench_verifier_cases --test package_paths` (R16 verifier + stdio/bridge regression, package paths — shared-path impact of the bridge and `bench_verify_support` change) | 20 + 2 passed | `test-bv1.log` |
| `cargo test --release --lib -- sweep_repeats_clock bench_verify_support verify_fixture` | 23 passed (the later run of the two touched modules: 9 passed, `test-lib2.log`) | `test-lib1.log`, `test-lib2.log` |
| `cargo build --release --bins` (default features, run **after** the tests) | PASS; `selftest_sweep_repeats` ELF aarch64 1,531,752 bytes | `build-release-bins1.log` |

The 12 new program checks (`checks/sweep_repeats_cases.rs`, each runs the actual binary with a cleared environment on a
private copy of the deployed tree): normal pass with the original output byte for byte and an unchanged tree · ignored
arguments · `-v` among other arguments · no `git` / unknown repository · missing deployed sweep script · changed log
line fails only N3 · reader raising on import fails only N4 and is imported once, late, as `render_bench_section` ·
reader exit stops after ten lines · unusable owner interpreter · missing conformance program fails only G ·
concurrent claim (both calls started before the first claim; one winner) · inherited owner-root override fails exactly
four named checks. Every check asserts the temporary root is empty afterwards. Data: `checks/data/sweep_repeats/normal_stdout.txt`
(the original's captured output). Full `cargo test` was not rerun: the changed shared paths have their focused checks above.

Failures met and corrected (kept): the first version of the claim-race check measured the instants at which the two
claim programs ran (a few hundred microseconds each, 13 ms apart) and failed with "did not overlap" — in the real chain
the second call refuses at the existence test, not at `mkdir`; the check was redesigned to prove that both calls had
started before the first claim (`logs/test-sr1.log` shows the failure, `test-sr2.log` and `test-final-sr.log` the
redesign). A dead-code warning of an unused helper method was removed before Clippy.

Resume if a gap is found: the deliverables are `src/selftest_sweep_repeats.rs`, `src/sweep_repeats_*.rs`,
`src/bin/selftest_sweep_repeats.rs`, `checks/sweep_repeats_cases.rs`, `checks/data/sweep_repeats/`, the edits named above and
the two manifest entries in `Cargo.toml`. Root owns the launcher, the build and distribution wiring and the SKILL text.

### R17 v2 correction — catchability follows the real class hierarchy (after Root's original-first finding)

Finding (kept): the held v1 port decided what N4's `except Exception` catches with a name list in
`selftest_sweep_repeats.rs::is_base_exception` (`SystemExit`, `KeyboardInterrupt`, `GeneratorExit`,
`OwnerBridgeError`). A custom `BaseException` subclass raised by the reader was therefore caught as a failed check
and the run went on. Root ran the original once on that input (a reader whose `parse_report` raises
`class R17FlowAbort(BaseException)` with a two-line Korean message): exit 1, stdout 1043 bytes (10 PASS, 0 FAIL), stderr
875 bytes with the qualified `render_bench_section.R17FlowAbort` and both message lines, no failure summary. Held v1:
exit 1, stdout 8926 bytes (74 PASS, 1 FAIL), stderr 164 bytes. Both raw results stay under Root's
`r17-baseexception-reference/` and `r17-baseexception-candidate-v1/`; the first failure is not rewritten.

Fix (smallest typed design, no name list, no new interpreter or eval):

* `owner_bridge.py::_failure` adds one fact, `"is_exception": isinstance(error, Exception)`, asked of the real owner's
  interpreter for the exception that was actually raised, in both the `SystemExit` and the general record.
* `Raised` (`conformance.rs`) gets a private `outside_exception` flag (default `false` for everything built in Rust),
  `Raised::outside_exception()` and `Raised::is_exception()` (`!outside_exception && !is_system_exit()`).
  `owner_bridge::raised_of` sets the flag from the record. The qualified class, message and `repr` are untouched.
* `OwnerBridge::call` is now a thin wrapper over `call_split`, whose outer `Err` is the transport (the session broke)
  and whose inner `Err` is an exception the owner raised. `parse_sweep_report` returns
  `Result<Result<SweepReportParse, Raised>, Raised>`: a transport failure stays fatal (outer `?`), and N4 catches an inner
  exception only when `raised.is_exception()`. Every other caller of `call` behaves as before (the wrapper flattens).
* Unchanged: the lazy reader session at the first report check, the plain module name and `sys.modules`, qualified
  class and multi-line message, `SystemExit` handling, raw fragment/ACK/stdio behaviour, cleanup, the R16 defaults.

Registered regressions (`checks/sweep_repeats_cases.rs`, now 14 program checks): the original-first input
(`a_custom_base_exception_…`: exit 1, the first ten lines, 1043 bytes, 10 PASS / 0 FAIL, no failure summary, exact
stderr `Traceback…` + `(native: Python stack frames are not reproduced)` + `render_bench_section.R17FlowAbort: R17 원본 중단`
+ `두 번째 줄`, empty temporary root) — it failed on v1 as Root's raw shows and passes now; and the hierarchy control
(`a_custom_exception_subclass_…`: `class Leaf(Middle)`, `Middle(Exception)` with the same two-line message is a caught
failed check, 74 PASS / 1 FAIL, the whole two-line detail printed, run continues). A `KeyboardInterrupt`, a custom
`BaseException` or any other class is classified by inheritance only; no new name was added anywhere.

Checks (managed Rust 1.99.1, offline, locked; logs in the worker's check directory `logs/`):

| Check | Result | Log |
| --- | --- | --- |
| `cargo test --release --test sweep_repeats_cases` | 14 passed / 0 failed, 18.60 s | `test-v2-sr.log` |
| changed-scope impact: `--lib` 158 · `bench_verifier_cases` 20 (stdio/ACK/qualified-class regression of R16) · `conformance_api_transport` 10 · `conformance_cli_cases` 12 · `conformance_helper_cases` 12 · `package_paths` 2 | all passed | `test-v2-impact.log` |
| `cargo fmt -- --check` | PASS | `fmt-check2.log` |
| `cargo clippy --all-targets -- -D warnings` | PASS | `clippy3.log` |
| `cargo build --release --bins` (after the tests) | PASS; `selftest_sweep_repeats` 1,531,792 bytes | `build-release-bins2.log` |

Reused, unchanged results (no rerun, source impact stated): Root's original-first default/noGit/missing-source and the
installed and independent checks cover paths this change does not touch (the printed lines, the copies, the groups other
than N4, the interpreter-failure and `SystemExit` paths); the v1 experiments of `logs/orig-*` and the strptime probe are
about unchanged behaviour. The original and the official benchmark were not run again. No file was added (600 files).

Remaining limits are those of v1: the shim text and the hint reader are still Python, reached through the owner
bridge; Python stack-frame lines are not reproduced; the 170/120 s waits were not exercised; no speed claim. The whole task
remains INCOMPLETE (S1–S7, harness, six profiles, actual single-4/multi-2 Long E2E).

### R17 v3 correction — owner hierarchy facts are authoritative, not the class name

Remaining gap (kept): the v2 `Raised::is_exception` was `!outside_exception && !is_system_exit()` and `is_system_exit`
compared the bare class name. An owner's ordinary `class SystemExit(Exception)` was therefore ended as if it were the
builtin although Python had said `isinstance(error, Exception)` is true. Root froze the original first
(`r17-systemexit-name-reference/`: reader defines `class SystemExit(Exception)` and `parse_report` raises it; N4 catches
it, exit 1, stdout 8922 bytes = 74 PASS + one failed N4 check, stderr 164 bytes) and then the held v2 once
(`r17-systemexit-name-candidate-v2/`: FAIL, stdout 1043 bytes, stderr 29 bytes — the run ended and printed the reason).
Both raw results and the independent source note stay as they are.

Fix:

* `owner_bridge.py::_failure` sends both `isinstance` facts for the exception actually raised:
  `is_exception: isinstance(error, Exception)` and `is_system_exit: isinstance(error, SystemExit)` (the builtin;
  a class that only carries the name is not an instance of it).
* `Raised` holds `owner: Option<OwnerHierarchy { is_exception, is_system_exit }>` instead of the v2 `outside_exception`
  flag. `Some` = what the owner's interpreter answered; `None` = built in Rust. `is_exception()` and `is_system_exit()` return
  the owner's fact when there is one and fall back to the class name **only** for a Rust-built error (a genuine Rust
  `SystemExit` still behaves as before). `owner_bridge::raised_of` sets `Some` only when the record carries both booleans.
* Every CLI path that asked `is_system_exit()` (`bench_verifier_cli`, `conformance_cli`, `broad_search_selftest_cli`,
  `selftest_sweep_meta`, `conformance_checks`) now gets the authoritative answer; no formatter compares a name. The real
  builtin keeps its string reason / status behaviour (R16 tests); a `BaseException` named `SystemExit` takes the
  traceback path with its qualified class (`render_bench_section.SystemExit`) and both message lines.
* No name list anywhere, no new interpreter, evaluation or object support; transport and owner failures stay separate
  (v2 `call_split`); lazy session, module name, ACK/stdio and import timing are untouched.

Registered regressions (`checks/sweep_repeats_cases.rs`, now 16 program checks) and one unit test:
`an_ordinary_exception_that_is_named_like_a_builtin_…` (`class SystemExit(Exception)` — the frozen input: exit 1,
stdout exactly the normal output with N4's hint line failed and without the closing line, 8922 bytes, the one-failure summary
on stderr, empty temporary root; and `class KeyboardInterrupt(Exception)`, the same classification by inheritance);
`a_base_exception_that_is_named_system_exit_…` (`class SystemExit(BaseException)`: ten lines, traceback with
`render_bench_section.SystemExit: …` and both lines, no quiet reason); `conformance::tests::an_owners_hierarchy_facts_…`
(owner facts override the name; a Rust-built `SystemExit` still falls back). The v2 FAIL of the first is Root's raw
(1043/29); the existing custom-`BaseException` test and the genuine-`SystemExit` test still pass unchanged. A first run
of the new check failed on my own expectation (it kept the closing PASS line); the program output was right and
the test helper was corrected (`logs/test-v3-sr.log` keeps the failure, `test-v3-sr2.log` the pass).

Checks (managed Rust 1.99.1, offline, locked; logs in the worker's check directory `logs/`):

| Check | Result | Log |
| --- | --- | --- |
| `cargo test --release --test sweep_repeats_cases` | 16 passed / 0 failed, 35.51 s | `test-v3-sr2.log` |
| impact: `--lib` 159 · `bench_verifier_cases` 20 · `broad_search_selftest_cases` 17 · `conformance_api_transport` 10 · `conformance_cli_cases` 12 · `conformance_helper_cases` 12 · `conformance_import_cache` 4 · `package_paths` 2 | all passed | `test-v3-impact.log` |
| `cargo fmt -- --check` | PASS | `fmt-check3.log` |
| `cargo clippy --all-targets -- -D warnings` | PASS | `clippy4.log` |
| `cargo build --release --bins` (after the tests) | PASS | `build-release-bins3.log` |

Final default build: `selftest_sweep_repeats` **1,532,064 bytes**, sha256 `a9bab6f7486c071d49186a3eb489592e970ead2149b8ec6ddba47e64902eead2`.
Bookkeeping correction: the v2 text above states 1,531,792 bytes for the v2 binary, whereas Root's held v2 binary is
1,532,648 bytes with a matching hold hash. That figure was my oversight (I took it from an earlier listing and did not
re-read it from the final build); I do not know why the text size differed and do not explain it. The held artifacts are
unchanged; this size is read from the actual final default build.

Reused (not rerun, source impact stated): Root's original-first default/noGit/missing-source, installed and independent
checks, the v1 `logs/orig-*` experiments and the strptime probe cover paths this change does not touch; the original and the
official benchmark were not run. No file added (600 files). Limits as before: the shim text and the hint reader are Python
behind the owner bridge, stack-frame lines are not reproduced, the 170/120 s waits were not exercised, no speed claim.
The whole task stays INCOMPLETE (S1–S7, harness, six profiles, actual single-4/multi-2 Long E2E).

### R17 v4 correction — the `SystemExit` payload is typed, not read from `repr`

Separate finding (kept): after v3 the CLI still decided a `SystemExit`'s ending in
`bench_verifier_cli::exit_status_of` by stripping the prefix `SystemExit(` from `repr()` and parsing an integer. An owner's
real subclass (`class R17RealExit(SystemExit)`) has the repr `R17RealExit(17)`, so its code was lost and its integer was
read as a string reason. Root froze the original first (`r17-realexit-reference/`, one call each at N4, ten lines /
1043 bytes in every case): `R17RealExit(17)` → exit 17, stderr empty; `R17RealExit(None)` → exit 0; `R17RealExit(True)` → exit
1. Held v3 (`r17-realexit-candidate-v3/`): all three exit 1 with one newline on stderr. Raw results stay as they are.

Fix:

* `owner_bridge.py::_exit_payload(code)` reports the real `error.code` as a record: `{"status": int}` for `None` (0) and
  for an integer including `bool` (`int(code)`), `{"message": str(code)}` for anything else. The session still holds no
  rule beyond that Python fact; `_failure` emits it as `exit` (the old integer `exit` field is gone; nothing else read it).
* `conformance::SystemExitOutcome { Status(u8), Message(PyStr) }` and `Raised::{with_system_exit_outcome,
  system_exit_outcome}`: the owner's payload when it reported one; for a `SystemExit` built in Rust the message as a
  reason (the existing fallback). `owner_bridge::exit_outcome_of`/`process_status` build it from the record. The status is
  what the operating system sees, confirmed against CPython 3.12.3 (`logs/python-exit-status-probe.txt`): the integer's low
  eight bits (273 → 17, -1 → 255, -256 → 0, 2147483653 → 5), and 255 for an integer that does not fit a C `long`
  (`2**63`, `2**70`; `PyLong_AsLong` fails and answers -1). No cap was invented.
* `bench_verifier_cli::run_verifier` uses `system_exit_outcome()` for an `is_system_exit()` error: a status ends the run
  quietly with it, a reason (the empty string too) is written with a newline and the status is 1. `exit_status_of` and the
  `repr` parsing are deleted. The other CLIs (`conformance_cli`, `broad_search_selftest_cli`) keep their string-reason
  path, whose errors are Rust-built.
* Unchanged: authoritative hierarchy facts (v3), transport/owner separation, ordinary `Exception` named `SystemExit` caught,
  `BaseException` named `SystemExit` traceback, genuine `SystemExit` string, qualified multi-line messages, stdio/ACK, lazy
  import, cleanup, R16 defaults. Arbitrary owner objects as `code` are printed by their `str` (what the interpreter does)
  but are not a newly required, separately verified case.

Registered checks (`checks/sweep_repeats_cases.rs`, now 17 program checks): `a_real_system_exit_subclass_…` runs eight
codes on `class R17RealExit(SystemExit)` — Root's frozen `17`, `None`, `True` (each: exit 17/0/1, stdout the first ten lines
= 1043 bytes, stderr empty) and `False`, `273`, `2 ** 70`, `''` (exit 1, stderr `\n`), `'R17 reason'` (exit 1, stderr
`R17 reason\n`) — with an empty temporary root each time; the v3 FAIL of the first three is Root's raw. Unit tests: two in
`owner_bridge` (status normalization table, payload shapes) and an extension of the `Raised` test.

| Check | Result | Log |
| --- | --- | --- |
| `cargo test --release --test sweep_repeats_cases` | 17 passed / 0 failed, 35.71 s | `test-v4-sr.log` |
| `--lib -- owner_bridge conformance::tests` | 13 passed | `test-v4-lib.log` |
| impact: `--lib` 161 · `bench_verifier_cases` 20 · `broad_search_selftest_cases` 17 · `conformance_api_transport` 10 · `conformance_cli_cases` 12 · `conformance_helper_cases` 12 · `conformance_import_cache` 4 · `package_paths` 2 | all passed | `test-v4-impact.log` |
| `cargo fmt -- --check` | PASS | `fmt-check4.log` |
| `cargo clippy --all-targets -- -D warnings` | PASS | `clippy5.log` |
| `cargo build --release --bins` (after the tests) | PASS | `build-release-bins4.log` |

Final default build, read after the build: `selftest_sweep_repeats` **1,534,616 bytes**, sha256
`2ea65537887ee4b68f934d82d500a7202abc8694d04f7a80f0678e291643b436`. (The log names `fmt-apply10`, `check-v4b` belong to a
first attempt of this step whose helper script failed on a quoting error before writing anything; they are kept as they
are and the step was redone under new names.)

Reused, not rerun: Root's original-first cases, installed/independent checks, v1 `logs/orig-*` experiments, the strptime
probe; the original and the benchmark were not run. No file added (600 files). Limits as before: shim text and hint reader
are Python behind the owner bridge, stack-frame lines are not reproduced, the 170/120 s waits were not exercised, no speed
claim. The whole task stays INCOMPLETE (S1–S7, harness, six profiles, actual single-4/multi-2 Long E2E).

### R17 v6 — test-fixture premise correction (installed layout), production source unchanged

Finding (kept): the canonical installed run of the selected Cargo checks had 216 passed and one failure,
`sweep_repeats_cases::a_missing_git_is_named_before_any_fixture_and_an_unknown_repository_after_it`, line 424: expected
`Some(2)`, got `Some(0)` (raw: Root's `r17-canonical-checks/affected.stdout` and `affected.stderr`). The no-git half passed.
The unknown-repository half ran `CARGO_BIN_EXE_selftest_sweep_repeats`, which in a canonical installation sits in
`<repo>/.claude/skills/adversarial-benchmark/rust/target/release/`; the binary therefore legitimately found that
repository and ran all 75 checks. In the prototype layout the Cargo target directory is outside an installed tree, so the
old premise held by accident. This was a premise error of the test, not a runtime bug: repository discovery and the
expected status 2 are unchanged and were not weakened.

Fix (only `checks/sweep_repeats_cases.rs` and this record):

* `run(setup, scratch)` stays the run of Cargo's binary and now delegates to `run_binary(program, setup, scratch)`
  (same cleared environment, same bounded `ETXTBSY`-only start retry).
* The unknown-repository half copies the program with `copy_detached` (a new inode, never a link) to
  `<scratch>/isolated-program/selftest_sweep_repeats` and runs that copy; the parent directory is not named `release`
  (asserted), so the installed layout is false by construction whatever the caller's `TMPDIR`. The assertions are
  unchanged: status 2, empty stdout, the named `ERROR BENCH_VERIFY_REPO_ROOT …`, empty temporary root.
* New control `an_installed_layout_program_finds_its_repository_without_the_override`: the same program as a new file in
  the release directory of a private tree, run with no override, finds that tree and prints the normal 75-check output
  (status 0, stderr empty, stdout byte-identical to the original's), empty temporary root. It is the positive case of the
  installed layout, in the prototype as in a canonical installation.
* No path is hard-coded; nothing skipped, ignored or lowered.

Focused run (managed Rust 1.99.1, offline, locked): `cargo test --release --test sweep_repeats_cases -- a_missing_git
an_installed_layout` → 2 passed / 0 failed, 17.75 s (`logs/test-v5-focus.log`); `cargo fmt -- --check` PASS
(`fmt-check5.log`, `fmt-check6.log` after a touch); `cargo clippy --all-targets -- -D warnings` PASS (`clippy6.log`,
`clippy7.log`); `package_paths` 2 passed (`test-pp6.log`). The earlier prototype pass of the old test and Root's canonical
failure are both retained as evidence. The full 17-check target, the original runs and the official benchmark were not
rerun; Root reruns the failing check and the required source checks in the canonical tree. The target now has 18 program
checks.

Artifact sequence, read in order (historical record kept): in the v4 last tool call the default build and its `stat` gave
1,534,616 bytes / sha256 `2ea65537…`; the later `PORTING.md` append and the `package_paths` run relinked the release product
to 1,535,136 bytes / `7ef179…`; Root's default build after that test (0.244 s, no source change) restored exactly `2ea…`. So the
earlier size difference was the order of my commands (a test after the default build), not corruption. This step ran the
explicit default `build --release --bins` last, after all tests and `package_paths` (`logs/build-release-bins5.log`): final
`selftest_sweep_repeats` **1,534,616 bytes**, sha256
`2ea65537887ee4b68f934d82d500a7202abc8694d04f7a80f0678e291643b436`, identical to the held v5; the statistics are in the worker's
`r17-final-v6-default-build.json`, written after the last Cargo command. This PORTING edit followed it and no Cargo command
ran afterwards. Production Rust and `Cargo.toml` are unchanged; the package still has 600 files. The whole task stays INCOMPLETE.


## R17 controller integration and whole-program comparison

The actual `selftest_sweep_repeats.sh` launcher, distribution call, policy edge, two operative SKILL references and
explicit build now use the native repeat verifier. The build runs 23 startup checks. All 75 scenario marks, actual
deployed owners, fake-load provenance, local fixture work and cleanup are retained. This is not Python-zero: the
shim strings, the real hint reader and deployed campaign/inline Python owners remain explicit later-stage work.

Root compared the three frozen default/no-git/missing-source cases, four copied-installation cases, one custom
BaseException, one ordinary Exception named SystemExit, and three scalar payloads of a real SystemExit subclass.
The exception-name/hierarchy and payload/repr failures were corrected. The ordinary Exception continues as one failed
N4 check; the custom BaseException stops after ten prior marks; genuine SystemExit status 17, None and True produce
17, 0 and 1 with no diagnostic. The bridge now carries actual hierarchy facts and a typed status/message outcome.
Valid earlier cases were reused across reviewed source changes. Independent validation compared two separately
frozen N4/git faults and reviewed the saved new counterexamples and final artifact without replaying originals.

Canonical validation passed fmt, all-target Clippy, 218 current selected cases and 23 startup checks. The 218 are
216 valid cases from the first invocation, one corrected negative and one added positive; they are not a new single
218-case invocation. The initial unknown-context test executed Cargo's binary inside an actual installation and
wrongly expected status 2; its status-2 assertion was retained and the binary was detached into a non-release path.
An implicit installed-layout positive control was added. The first failure and valid earlier checks are preserved.
Fresh Git-index installation used 823 files (600 Rust package files), ran all 218 selected cases in one invocation,
passed 23 startup checks and restored default products. Whole distribution passed 131/131 in 250.561381 seconds.
This is a fresh local path, not a physical second PC, subnode or x86 run.

Artifact sequencing was also corrected. The author's recorded v4 default build really was 1534616 bytes with its
reported SHA; the subsequent PORTING append and package-path test relinked a different release executable, which
Root first held as v4. The raw event sequence proves this order. Root's deterministic final default build after all
tests restored the exact earlier default product in 0.244 seconds with source600 unchanged, held as v5. The later
test-only v6 has the same production source and default binary. This is verification/build restoration by Root;
the Rust source author remains the same Native Claude session. Canonical and fresh validation likewise finish
with an explicit default build after every test. Historical size statements and artifacts are not deleted.

Timing used one actual whole verifier, one warm pair and five alternating measured pairs: 12 new CLI records,
two warm and ten measured. Observer clone preparation was outside the timer; the program's Git, private fixtures,
deployed owners and embedded programs, IO, 75 checks, full output and cleanup were inside. Every complete output
and exit matched the frozen original, and the private 235-file closure stayed unchanged.

| Whole verifier | Python median seconds | Rust median seconds | Python/Rust |
| --- | ---: | ---: | ---: |
| repeat sweep | 17.625047333 | 17.522088685 | 1.005875935 |

The small observed difference does not establish statistical significance. This boundary remains in the final
Rust-time review with the preceding limits. No separate pure fragment, LLM, GPU, serving or six-profile result is
inferred. The original 68-line roofline aggregation property was already ported as native roofline self-test R1
with the same tensors and exact figures; its current passing startup result is reused instead of reimplementing it.
Its legacy unittest frontend remains a reference, without a claim of generic unittest CLI compatibility.

Evidence: `r17-root-default-product/`, `r17-realexit-default-product/`, `r17-installed-candidate/`,
`r17-canonical-checks/`, `r17-canonical-continuation/`, `r17-canonical-installed-positive/`, `r17-packaging/`,
`r17-distribution/`, `r17-benchmark-results/`, `r17-default-product-restore/`, `r17-roofline-reuse.json`, and the
independent `final-v4/`, `default-product-v5/`, `install-layout-v6/` and `benchmark-review/`. Python source frames
are projected separately from qualified/multiline diagnostics. The 170/120-second waits were not replayed, the
private time reader consumes the deployed producer's ASCII UTC form, and arbitrary Python object/stream mutation
is not proved. S1 inline/interop work, later stages, six actual profiles and Long E2E remain incomplete.


## R18 sweep index and meta production

Source: the 634-line Python program embedded in the deployed `sweep_bench.sh` (the one production site that
assembled `sweep_index.json` and its `meta`). It is now one native owner: library `assemble_sweep_meta`
(orchestration, environment, IO, index layout), `sweep_meta_scan` (the original's naive text expressions as hand-written
matchers: the "YAML" and env readers, the engine-log patterns, the speculative-decoding declaration), the program
`assemble_sweep_meta` (`--self-test` start-up check) and the thin launcher `assemble_sweep_meta.sh` that Root owns
(`ASSEMBLE_SWEEP_META_BIN` override, else `../rust/target/release/assemble_sweep_meta`, status 2 when missing).

Source mapping. Environment and level/int parsing with Python `int()`/`str.split()`/`isdigit()` semantics (existing
`pynumber`, `pystr` tables); `read()`/`json.load` fallbacks with the original's `except` sets (`pyfs`, `pyjson`);
image / version / build / native serve proof precedence; the mandatory GPU model (stderr + status 2); tensor-parallel
derivation (a single-node manifest counts one node); model axis from the checkpoint basename; the measured-first
quantization, KV dtype, attention and MoE axes with declaration and mismatch; the spec declaration and its
reassembly inheritance / retro mark; measured-node provenance; the tool, tool-image and dataset sidecars; levels,
truncation lines, lite block, repeat summary and clamp boundary, bench knobs; fresh UTC clock for a measurement and the
inherited exact `generated_utc` for a reassembly. Existing owners are called directly and nothing is copied:
`bench_naming::gpu_key`, `repeat_axis::summarize`, `lite_metrics::build` / `read_lite_verdict`. JSON is
`json.dump(index, f, ensure_ascii=False, indent=2)`: no final newline, the file opened (and so emptied) before the text
is made, and a lone surrogate leaves exactly the encoder chunks that preceded it on disk (position counted in the
failing chunk, a list element carrying its separator; confirmed against CPython).

Ending semantics: `sys.exit(2)`, `SystemExit("reason")` (reason + newline on stderr, status 1) and uncaught
exceptions (traceback marker line, `Class: message`, status 1; `RepeatAxisError` and `JSONDecodeError` carry their
module as Python prints them). Source stack frames are the only projection. A prior index survives every failure that
precedes the open for writing. The owner's `SystemExit` raised from the lite metrics module is not an `Exception` and
ends the run; every other lite failure is recorded as `lite_metrics 산정 실패: <message>`.

Intended differences from the original: `SDIR` is not read (it only located Python helpers); no Python is started,
imported or found on `PATH`; the Python interpreter start-up cost disappears. The list `ENV_NAMES` is the one place
that names the variables the producer reads; the deployed boundary removes exactly those from an inherited
environment, and a check proves the program asks for no other.

Checker integration (intended binding changes, nothing deleted or weakened):

* `selftest_sweep_meta` no longer extracts or executes a Python heredoc. `SweepMeta::load` binds to the deployed
  producer (`sweep_meta_deployed`): `sweep_bench.sh` must call `bash "$SDIR/assemble_sweep_meta.sh"`, no hidden Python
  program that writes `sweep_index.json` may remain, the launcher must exist and follow the install convention, and
  `launcher --self-test` must answer; any failure is a `SystemExit` before the first scenario. Every fixture is one real
  run of launcher and program with exactly the producer variables; all C1~C10, S1~S10, W1~W7, RC1~RC3 and real-log
  scenarios, labels and order are unchanged. The Python session `heredoc` operation was removed from the owner bridge,
  so the meta check no longer starts a Python session (two owner-interpreter checks now assert that it is unaffected).
* `selftest_lite_report` L8 builds its `meta` through the same boundary; L10's tool-token spelling is read from what the
  deployed producer writes for a level whose measurement names no tool instead of from the script text.
* `conformance_full_superset_lite`: the old static rows (`"key": _built.get`, `spec_from_file_location`) are replaced by
  eight rows that read the actual production owner and caller and one real output: the caller binding, the installed
  launcher and owner source, the five `sweep_index.lite` keys (each needs the key in the owner source and in a real
  producer output built from the D1 fixture with a stored verdict), and the direct owner calls with the raw verdict
  carried unchanged. The report has 43 rows (was 41); a failed real run adds a row of its own.
  `sets_key_from_built` and its unit test were removed with the shape they served.
* Source/COPIES closures updated: the repeat sweep fixture copies the launcher and the release program (67 -> 69 files),
  its private tree copies `assemble_sweep_meta.rs` as a conformance source, and the conformance installation copies it
  too and points `ASSEMBLE_SWEEP_META_BIN` at the program built from this crate. Hard links of programs into private
  conformance installations were replaced by detached copies.

Registered checks (`checks/sweep_meta_cases.rs`, 31 tests): 23 frozen original cases (inputs, environment, whole
stdout/stderr/status/index and the file delta of the run, run through the real program on clones, fixture root and a fresh
clock projected; the clock is checked against the interval of the call); the original status classes 0/1/2, a
reassembly, an uncaught exception and a `SystemExit(message)`; the producer reads only `ENV_NAMES` and each of them is
read; scenarios the originals do not reach (tool, tool-image and dataset sidecars, dataset order and duplicate
sampling, non-object sidecars, native serve proof variants, image/digest inheritance, integer and `%02d` semantics of
levels, tensor-parallel derivation, model fallback and node provenance, version precedence, backends and warnings, the
lite block and the owner's `SystemExit`, repeat request and clamp, knob record inheritance, required variables, open
and encode failures with partial files, prior-index survival); the program with no `PATH`, its traceback form, its
start-up check, and the installed launcher (override, missing and non-executable program = status 2).
`conformance_cli_cases` gained four binding checks (caller gone, hidden dead program, launcher gone, program that does
not start) and `bench_verifier_cases` four binding checks and the two interpreter checks above.

Commands and results (managed Rust 1.99.1, offline, locked, own target): `cargo fmt --check` clean; `cargo clippy
--all-targets -- -D warnings` clean; `cargo test --release --no-fail-fast` 125 targets, 575 tests passed, 0 failed
(lib 166, `sweep_meta_cases` 31, `bench_verifier_cases` 23, `conformance_cli_cases` 16, `sweep_repeats_cases` 18 with the 75-check
macro run through the real producer, `package_paths` 2); the 38 frozen originals match byte for byte through the worker
comparison script as well. The first full run failed on stale release programs in the staged installation (the old
conformance program) and on three conformance and one verifier expectation that this change moves (the 36/44 row count of
a failed real run, the interpreter checks); those were corrected, not skipped. The tests run against an installation staged
by the worker from the canonical closure, the four Root coupling snapshots, these sources and detached programs; the
canonical checkout was only read.

Limits: this is the one production site of the index, not the remaining inline Python of other stages (S1 inline and
helper candidates, S2-S7, six profiles, Long E2E). `\w` follows Rust's alphanumeric class, which differs from CPython's
`str.isalnum()` for combining marks only. Arbitrary Python object semantics beyond the JSON value kinds the sidecars hold
are not modelled. No performance claim is made here; the Root-timed boundary comparison is separate.

Final default `--release --lib --bins` build ran after the last Cargo test and no Cargo command ran after it: 49 programs,
73,352,920 bytes; `assemble_sweep_meta` 1,459,936 bytes (sha256 `10e658dc…186a5c`), `selftest_sweep_meta` 1,186,096,
`selftest_sweep_repeats` 1,529,840, `conformance_full_superset_lite` 1,205,160; the package has 795 files (272 Rust files);
the new production files are `assemble_sweep_meta.rs` (1,422 lines), `sweep_meta_scan.rs` (884), `assemble_sweep_meta_cli.rs`
(341) and `sweep_meta_deployed.rs` (206). The whole task stays INCOMPLETE.


## R18 correction: Python word class and the second measurement read

Two behaviours Root reviewed were wrong in the first R18 handover and are fixed.

1. **`\w`, `\W` and `\b` are CPython's, not Rust's.** The earlier text above called the word-class difference a known
   limit; it was a defect. A new original-first case (one more full call of the original program, frozen as
   `checks/data/sweep_meta/unicode-word-boundaries`) places U+0345 (combining Greek ypogegrammeni, which Rust counts
   as alphanumeric and CPython 3.12.3 does not: `isalnum()` false, `re` `\w` false) before `v8.9.10`, inside `Using
   MARLIN <mark> MoE backend` and in `num_speculative_tokens<mark>3`. The original gives `vllm_build='8.9.10'`,
   `moe_backend='NA'`, `spec_declared_k=3`; the held first version gave `'NA'`, `'marlin'` and null. No existing exact
   helper was found (`pystr` had only decimal digits, spaces and the `isdigit` extras), so `pystr::is_python_word`
   now reads `src/unicode_alnum_table.rs`: the 747 inclusive ranges of `str.isalnum()` (plus the underscore), enumerated
   once over all 0x110000 code points by the dev-time generator `gen_unicode_alnum.py` under CPython 3.12.3 /
   Unicode 15.0.0 (the generator refuses any other version; notice row added to `notices/NOTICE.md`). It is
   compatibility data, not a domain rule, and no mark class is approximated. `sweep_meta_scan::is_word` uses it; no Python
   runs. Regressions: the frozen case (24 frozen cases now), a producer scenario with the three expressions, a scanner
   check of characters that differ between Rust and CPython, and a table unit test. `render_report.rs` still has its own
   `is_alphanumeric` use for a different expression; it was not part of this unit and is untouched.
2. **Three reads of the first readable `measured.json`.** The original loops over the levels three times (tool block,
   server-alive block, level records). The first version kept the first read and reused it for the second block while
   its comment said it read again. `append_sidecar_provenance` now calls `first_readable` a second time in the
   original's place. A check makes `measured.json` a named pipe that answers every open with the next document: the
   tool comes from read one, `server_alive_at_bench_end` / `boundary_exemption` from read two and the level record from
   read three.

Counts after this change (fmt clean; Clippy clean; only the affected targets rerun, the earlier full 575/0 run stays
valid for unrelated sources): lib 167 (was 166), `sweep_meta_cases` 34 (was 31), `package_paths` 2,
`conformance_helper_cases` 12 all passed; the 38 saved original cases still match byte for byte through the worker comparison
script, with no original re-executed. The whole task stays INCOMPLETE.

The explicit default `--release --lib --bins` build ran after the last Cargo test and no Cargo command ran after it: 49
programs, 73,354,424 bytes; `assemble_sweep_meta` 1,459,400 bytes (sha256 `b2269ea8…6af55`), `selftest_sweep_meta` 1,186,008;
the package has 804 files (273 Rust files). Earlier byte counts and hashes in this file describe the previous build.


## R18 test-driver correction: the FIFO regression

The regression I added for the second `measured.json` read (`the_first_readable_measurement_is_read_separately_by_each_of_the_originals_three_loops`) was
unbounded and **did hang**. Its writer opened the pipe by successive `fs::write` calls with no reader-close handshake, so
documents could coalesce into one read and a later open waited forever for a partner after the writer had ended. My first
registered run stayed in that state until the 400-second background limit; the later cleanup-only change (a non-blocking
drain loop) did **not** solve it, because `produce()` itself was the blocked call and the drain never ran. Root's own
probe of that exact driver timed out again on its first call and Root sent SIGTERM to the detached test process only
(not user cancellation, not capacity, not production): its receipts are Root's, the raw logs of my hung run and of its
rerun stay in the worker's log directory (`test-d-1`, `test-d-2`). The earlier "34 passed" counts therefore included a
test that was not sound; the production behaviour it targeted was not wrong and is unchanged.

Replacement (test driver only; no production, library, caller or expected data changed): the test
`a_second_read_of_the_first_readable_measurement_sees_what_replaced_it_after_the_first_read` links `measured.json` to a
pipe. One writer thread opens the pipe once without blocking (retried on "no reader yet" only, 30-second deadline),
writes document one, atomically replaces the link by a regular document two **before** closing the pipe, then closes.
The first read stays on the pipe (document one, then end of file); every later read follows the regular file. On a
failed premise the writer releases a late reader until the run is done (bounded 10 seconds) and the test fails with the
reason; the writer thread is always joined. Assertions kept: the tool comes from read one, `server_alive_at_bench_end`
(false) and `boundary_exemption` from a later read, the level record from a later read. It proves a second read happens
after the first; it cannot tell the second from the third read. Negative control: a throw-away copy of the sources
with the first read cached for the server-alive block fails this test at the server-alive assertion in 0.01 s (log
`test-fifo-negative-control`; the copy was deleted); the real sources pass.

Run after the change: `sweep_meta_cases` 34 passed / 0 failed, `package_paths` 2, `fmt --check` and Clippy
`--all-targets -D warnings` clean (all `# exit=0` in the preserved logs). `src/`, `Cargo.toml`, `Cargo.lock`, `build.rs` and
`notices/` are byte-identical to the held v2 source; only `checks/sweep_meta_cases.rs` and this file differ.


## R18 correction: Python `str.lower()`, and a record correction

**Record correction for the first FIFO driver (history above is kept).** The earlier section said my first registered run
"stayed in that state until the 400-second background limit". The 400-second values are limits Native configured for its
commands; a configured limit does not by itself show that a process was stopped. What Root's receipt
`r18-fifo-regression-driver-termination.json` shows: Root sent SIGTERM to the first hung registered driver (PID 3842338).
The second detached replay of the same driver (PID 3857996) was run by Root under a 4-second supervisor; that is a Root
bound and not a Native time limit. The hang itself (successive pipe writes, no reader-close handshake, `produce()`
blocked) and the failure of the cleanup-only change stand as recorded.

**Lower case.** The original lower-cases at four producer sites: `_norm_measured` (`str.lower()` of the engine token),
the mismatch comparison (`d.lower()`), the model axis (basename `.lower()`) and the `off` sentinel of a speculative
declaration. The held version used Rust's `str::to_lowercase`, whose Unicode is newer than CPython 3.12.3's (Unicode
15.0.0). A new original-first case (frozen as `checks/data/sweep_meta/unicode-lowercase-boundary`; one more full call of
the original by Root, nothing replayed) uses U+1C89, unassigned in Unicode 15: the original keeps `x<U+1C89>` and
`<U+1C89>` (mismatch `no`); the held version produced U+1C8A. Fix: `pylower::python_lower` is CPython's `unicode_lower`:
the full lower-case mapping of every scalar (U+0130 becomes two scalars) and the locale-independent Final_Sigma rule
(a capital sigma is final when a cased character precedes it and none follows it, case-ignorable characters being
skipped on both sides). Its data, `src/unicode_lower_table.rs`, is generated by the dev-time `gen_unicode_lower.py` from
the interpreter itself: mappings from `chr(cp).lower()` for every scalar (1,431 single, 1 multi), and the Cased /
Case_Ignorable ranges (150 and 437) from CPython's exported `_PyUnicode_IsCased` / `_PyUnicode_IsCaseIgnorable`, reached
through ctypes (a missing symbol stops the generator) and cross-checked for every scalar against behaviour probes of
`str.lower()` on `X` + capital sigma. The generator refuses any other interpreter version. Notice row added
(Unicode License V3). Only the four producer/scanner call sites adopted it; `render_report` and other stages are untouched.

Checks: `checks/pylower_cases.rs` (3 tests) compares every scalar through one digest of the interpreter's answers and 600
Final_Sigma / one-to-many / newer-Unicode strings, all produced once by the interpreter (`make_lower_oracle.py`, data in
`checks/data/lowercase/oracle.json`), and shows Rust's own lower case really differs for U+1C89; two scanner/producer
scenarios (final sigma in a model name, U+0130 in measured and declared quantization, the `off` sentinel); the frozen
whole-IO case (25 frozen cases now). The oracle is a primitive probe, not a whole-producer call. Raw candidate runs on the
saved lowercase and Unicode originals and on the 38 saved cases match (`compare_single_reference.py`,
`compare_r18_reference.py`; outputs kept in the worker directory; no original was re-executed).

Counts: lib 169 (was 167), `pylower_cases` 3, `sweep_meta_cases` 36 (was 34), `package_paths` 2, all passed; fmt and
Clippy `--all-targets -D warnings` clean. Earlier evidence is reused: the full 575/0 run, Root's 39 whole-IO / installation /
checker results and the FIFO replays concern unchanged behaviour; the IO, read-count, typed-error, binding and math code
is unchanged. The whole task stays INCOMPLETE.

## R18 external acceptance: deployed assembly, packaging and measured CLI

The external supervisor accepted this unit's production assembly and checker connections, not the whole migration.
The current generator passed 40 original-first full-IO cases (the initial 38 plus the word/lower Unicode counterexamples);
an independent validator ran two other frozen original inputs against the candidate once each. Ordered whole index
bytes, both streams/status, fresh UTC within the call interval, exact inherited reassembly UTC and declared file effects
matched. The independent dataset cases preserve short-circuiting, symlinks and the existing index's hard-link identity,
link count and write times before an error. The word/lower tables and Final_Sigma properties were separately audited
against CPython 3.12.3 primitives; this is distinct from additional whole-producer execution.

The supervisor corrected its own observation premises without changing expected production results: escaped projection
markers, retained non-frame diagnostic payload, resolved invocation paths before changing directory, accepted the
launcher's literal `scripts/../rust` missing-program path, and reused the meta check's valid 386-byte stderr after
confirming it exactly matched the frozen original. Old failures and both hung FIFO-driver interventions remain in the
external records. The corrected finite FIFO regression passed five bounded driver replays; the first cached-read
negative control still fails the behavior assertion. No original verifier was used as a runtime fallback.

Delivery required one crate-root `.gitignore` with two exact mock-fixture exceptions: the parent repository's operational
manifest/log patterns had hidden 75 approved mock input files. Putting the exception inside the case catalog initially
made it look like a case, so it was moved to the crate root. Real operational files remain ignored. Three existing
producer-literal consumers/dependency copies now name this Rust owner, and the corresponding verification-plane edge
was moved in its closed ledger. Their naming/campaign rules were unchanged; no dead Python text was restored to satisfy
a stale source check.

Current canonical verification is 299 selected passes composed of 200 initial valid passes, two corrected missing-context
cases, 76 further valid passes, three cases after the fixture-catalog placement correction, and the remaining 18 repeat
cases. It is not a new whole invocation of 299. A fresh installation from the Git index contains 1,041 source files
(817 in this Rust package: 816 Native-authored inputs plus the tracking exception), passed the same 299 in one invocation
and all 24 native start-up checks, then restored default products after Cargo tests. The copied deployment's five checker
programs also passed with all 1,077 source/product files unchanged. Distribution currently satisfies 131 checks by reusing
128 unaffected results and actually rechecking the changed hint, campaign and runtime checks; the first full run's
129/131 FAIL and the later passes remain separate.

Official timing used the six cases fixed before implementation: one warm-up and five alternating rounds per side,
72 calls total (12 warm, 60 measured). The complete Bash-to-Python-stdin original producer and Bash-to-native launcher,
actual owner computation/IO/JSON/output/status are inside the timer; fixture setup and comparisons are outside. Every
call passed whole-IO quality. The medians of the six-case round means were Python 0.050131823166666666 seconds and Rust
0.0034124955 seconds: Python/Rust 14.690663523707698. Per-case median ratios range 12.2397332165 to 16.6860199326.
The independent saved-artifact audit recomputed all 72 records, their schedule, full IO/clock/file effects and aggregation
without rerunning products, originals, Cargo or timing. This is the assembly CLI on six fixed fixtures; it is not a
model-serving, whole-Long-E2E or statistical-significance claim. Earlier open Rust-time limits remain open.

This appendix was added after that saved-artifact audit's protected interval ended. It is a documentation-only change;
subsequent source-package checks and the final default product restore have their own execution records. Remaining S1
inline/interop work, the other stages, six main/sub profiles and actual single/multi Long E2E are INCOMPLETE.

## R44 — 캠페인 조회의 기본 경로를 프로세스 안의 T 읽기면으로 (repeat_axis · bench_preset)

상태: 이 범위의 자체검사 PASS(아래). Root 정상 입력·독립 c11·표준 190·Main/Sub·성능·R44 original15/saved26의 Root 판정은 미실시이며, 이 문서는 그 어느 것도 주장하지 않는다.

**범위.** `repeat_axis`·`bench_preset`이 `campaign_init`에 닿는 유일한 이음매 `CampaignOwner`(`repeat_owner.rs`)에서, 설정이 없고 소유자 파일이 검토된 바이트와 같으면 매 `open()`마다 Python 어댑터를 띄우고 357 KB 모듈을 컴파일하던 경로 대신 `role-contract`(T)의 읽기면이 같은 프로세스에서 답한다. 읽기 규칙·상수는 T에만 있다(`campaign.rs`: `module_site`, `derive_root_path`, `active_campaign_id`, `BOOTSTRAP`, `RESERVED_IDS`, 경로 텍스트 합성). B는 경로 선택·설정·`Failure`/프로토콜 변환만 한다. writer·relay·ACTIVE 쓰기·purge·state는 이식하지 않았다.

**경로 선택(열 때마다).** 1) 옛 `pyfs::is_file`이 먼저다 — 없음·비정규·ENOENT/ENOTDIR/EBADF/ELOOP/NUL/인코딩 불가는 `Absent`(프로그램을 묻지 않음), `stat`의 PermissionError는 그대로 전파. 2) `REPEAT_AXIS_CAMPAIGN_ADAPTER`(비어 있지 않음) 또는 `REPEAT_AXIS_PYTHON`(빈 값 포함 설정만 돼도) = `LegacyExplicit`: 옛 어댑터가 어떤 소스든 실행, 오류도 그대로. 3) 설정 없음 + 바이트 동일(`source_identity`, 기존 `include_bytes!`) = `NativeCurrentSource`: Python/Bash 자식 없음, 네이티브 오류·미지원 값에서 폴백 없음. 4) 다르거나 읽을 수 없음 = `LegacySourceVariant`: 기본 호환 어댑터(`REPEAT_AXIS_COMPAT_ADAPTER`, 래퍼가 export). 위치가 없으면 기존 `CampaignOwnerAdapterError`. `PythonCampaignOwner::new`는 명시 레거시 생성자로 그대로다.

**모듈 전역 루트.** 네이티브 세션의 루트는 호출자의 repo 루트가 아니라 `resolve(owner).parents[4]`다(심볼릭 링크는 따라가고, 루프는 `RuntimeError`, 5개 미만 성분은 `IndexError('4')`). `camp_id`는 형·진리값·텍스트만 관찰된다: 거짓값은 활성 캠페인, 텍스트는 코드포인트 위 lexical 경로 합성(절대 경로가 대체, `..` 유지, NUL·lone surrogate는 성공하는 경로 텍스트로 FS 인코딩 전까지 보존), 그 밖의 참 값은 `unsupported operand type(s) for /: 'PosixPath' and '<type>'`.

**경로별 증거(제품 안, 업무 출력 밖).** `REPEAT_AXIS_ROUTE_OBSERVER=<파일>`이 설정되면 열 때마다 JSON 한 줄이 덧붙는다: `route`·`reason`·`pid`·`owner_path`·`python_child`·`child_pid`·`opened`·`failure`. 열지 않으면 줄이 없다(BenchPreset 조기 반환은 기록 0). 요청한 기록을 쓰지 못하면 `CampaignRouteObserverError`로 열기가 실패한다(경로를 지어내지 않음). 임시 전송 `r44_owner_query_probe`는 출하 트리 밖(`W/probe`)이며 출하 52개에 들지 않는다.

**검사(자체 합성 픽스처 · Root 기록 행렬 재생 아님).** T: `topology_checks owner_queries` 6(모듈 루트·얕은 경로·루프·링크·형 TypeError·거짓값·경로 텍스트·surrogate)과 전체 T 20. B: `repeat_axis_cases::r44_routes` 6 — 기본 실행이 `NativeCurrentSource`이고 명시 어댑터(`LegacyExplicit`)와 stdout 바이트가 같음, `REPEAT_AXIS_PYTHON` 단독, 빈 인터프리터의 기존 실패, 주석·BOOTSTRAP 변형과 읽을 수 없는 소유자의 `LegacySourceVariant`, Absent 3형, 기록 실패, 그리고 실제 어댑터와의 차분(ACTIVE 8형·읽기 불가·링크 2형 × typed `camp_id` 20종, 모든 답·오류가 같음). 보존 oracle(owner82·FS42·preset67·variant60·API·selftest)은 하네스 환경만 기본 경로(어댑터 env 제거, 기본 위치만 지정)로 바꿔 기대 파일 수정 없이 통과했다. 소스가 다른 owner stub은 `LegacySourceVariant`로만 센다(네이티브 증거 아님). 저장소 조회가 필요한 일부 검사는 `W/tree`에 원본 모듈·스키마가 없어 읽기 전용 A를 `REPEAT_AXIS_REPO_ROOT`/`BENCH_PRESET_REPO_ROOT`로 가리켜 실행했다(첫 실행 6건 실패 원문 보존).

**하네스 변경(기대값 아님).** 기본 설정 케이스가 실제로 Native를 타도록 `run`/`command`가 어댑터 env를 제거하고 기본 위치만 준다. `an_unreachable_owner…` 시험은 이제 검토된 소유자가 어댑터 없이 네이티브로 답하므로 *다른 바이트* 소유자로 같은 단언(어댑터 미설정 시 `CampaignOwnerAdapterError`)을 유지한다. `resolve_topology_selftest.rs`의 격리 env 목록에 `REPEAT_AXIS_PYTHON`·`REPEAT_AXIS_COMPAT_ADAPTER`·`REPEAT_AXIS_ROUTE_OBSERVER`를 더했다.

**한계(주장하지 않음).** (1) 네이티브는 검토된 파일의 import 시점 계산 중 실패할 수 있는 위치 계산만 재현한다(PyYAML 부재 같은 환경 실패는 Python 경로의 몫이며 Root 라우팅 결정 3이 정의). (2) `camp_id`가 컨테이너면 비어 있음만 관찰한다. (3) `IndexError`의 `args`는 `Plain`(타입된 int 아님) — 이 와이어에서는 관찰되지 않는다. (4) `topology_parity_api.rs`의 B 전용 모듈 위치 계산은 R43 API ABI 보존을 위해 건드리지 않았다(중복 한 곳이 남음). (5) 공개 제품 핸들러와 맨 어댑터 계층은 같다고 단언하지 않는다(어댑터가 응답 없이 끝나는 S15형은 제품에서 `CampaignOwnerAdapterError`, 맨 어댑터는 종료 코드 7). (6) 성능은 측정하지 않았다. 잔여 S1 인라인/상호운용, 다른 단계, 여섯 main/sub 프로파일과 실제 단일/멀티 Long E2E는 INCOMPLETE다.

## R45 — R19 `bench_shell`(셸 인라인 파이썬 37지점)을 현재 정본 위로 최소 재기반, 승인된 값 의미 적용

상태: 이 범위의 자체검사 결과는 `W/result.json` 에 있다. Root 공개 통합·독립 c11·정본 채택·Main/Sub 전달·성능은 미실시이며 이 절은 그 어느 것도 주장하지 않는다.

**계보(관측).** 현재 정본(598453b)에는 `bench_shell`, R20 `report_unicode`, R21 `stdin_newline` 이 없다(git 이력에도 없음). 분리된 후속 계보(r19-source-v3 · r21-source-v1 · r35-source-v2)에는 있다. 세 계보의 `bench_shell` 14파일과 `json_stream.rs` 는 바이트 동일하고 `pyjson.rs`/`render_report.rs` 만 다르다(R20/R21 공유 수정). 이 후보는 v3 의 14파일만 가져오며 R20/R21 공유 수정과 옛 831 복사는 쓰지 않았다. R21 에서는 **시험 전용** 고정 시계(`checks/bench_verifier_cases.rs`)만 옮겼다: `date -u +%FT%TZ`/`%Y%m%dT%H%M%SZ` 두 형태만 대리하고, 고정 시계 FAIL 3/PASS 25 와 1초 전진 시계 FAIL 10/PASS 18 을 각각 단언한다. 제품 시계·명명 규칙은 그대로다.

**범위.** 새 `[[bin]] bench_shell`(일반 bin 52→53), `src/bench_shell.rs`+`src/bench_shell/*.rs`(7), 검사 3(`bench_shell_original_cases`·`bench_shell_edges`·`bench_shell_wiring`), 동결 자료 2 + 새 값 계약 자료 1, 등록 목록 병합(CHILD_OVERRIDES 13→14 등 현재 R43/R44 항목 보존), 필요한 픽스처 의존 목록과 `json_stream`/`assemble_sweep_meta` 보조. 호출부·런처·`build_rust.sh`·SKILL 은 Root 소유다.

**값 계약(사용자 승인, Root 확정).** `warm-fraction`: 정확히 두 인자, 각각 ASCII `[+-]?[0-9]+`(선행 0은 10진, 선택적 `+` 허용, 임의 정밀도). 계산은 기존 그대로(정수 정확 나눗셈, `max(total,1)`, `round(…,4)`, float repr, `OverflowError`). 그 밖의 입력(빈 값·공백·유니코드 숫자·16진·부동소수·지수·밑줄·이름·식·부호 중복·인자 개수 불일치)은 stdout 비어 있고 stderr 정확히 `[bench_shell] ERROR warm-fraction 은 10진 정수 둘(WARMUPS TOTAL_REQ)만 계산한다`, 종료 2, 평가·파이썬 폴백 없음. 기존 호출부의 Bash `TOTAL_REQ` 산술은 별개 경계이며 이 도우미가 정규화한다고 주장하지 않는다. `broad-measurement-ok PATH`: PATH 는 리터럴 argv 이고 기존 `measurement_ok` 와 같은 loader·JSON·형·진리값·오류 경로를 그대로 쓴다(소스 리터럴 분류와 SyntaxError 흉내 제거). NUL 은 실제 CLI argv 로 불가능하므로 라이브러리 문맥의 loader 오류만 시험한다.

**의도된 변경(이력, 동치 아님).** `WARMUPS=01`: 원본 SyntaxError → 값. 정당한 작은따옴표/개행 경로: 원본 SyntaxError → 리터럴 파일 읽기. 백슬래시 이스케이프: 원본이 다른 경로로 재해석 → 리터럴 경로. 선언 문법 밖의 임의 파이썬 소스: 평가 대신 거부. 저장된 원본 181행과 보충 원본 2건·원시 기록은 불변이며, 이 변경된 출력은 어떤 차분 PASS 로도 세지 않는다. 적용 가능한 181행은 모두 기대값 그대로 재생된다(warm/measurement 연산을 건너뛰지 않음). 새 값 계약 행은 `checks/data/bench_shell/value_contract.json` 에 따로 있다(후보 출력이 아니라 산술에서 유도한 기대값).

**호출 지점 37.** 현재 여섯 스크립트의 heredoc 인식 grep 은 39곳이다. 컨테이너 안 `lite_bench.sh` 의 `docker exec … python3 -c` 와 열 소유자를 import 하는 `broad_search.sh` 의 열 프로브 2곳을 제외하면 37 = 인라인 36 + 기존 Rust 소유자 interop 1(`run_bench.sh` 데이터 인자)이다. 카탈로그의 `original_line` 앵커는 물리 줄이 아닐 수 있어 연산 순서로 대응한다.

**한계.** 성능은 측정하지 않았다. 기존 호출부의 Bash 산술, 백엔드 개수 플래그, 임의 파이썬 소스 동치는 이 범위가 아니다. 가드된 환경에서는 `/tmp` 쓰기가 막혀 `broad_search_selftest_cases` 의 `/tmp` 대체 경로 시험 1건을 실행하지 못했다(그 시험 파일은 f42 이후 변경 없음).

## R47 — `campaign_init.py --active` / `--derive` 읽기 두 형태를 공개 CLI `campaign_read`로 (T 프레임 지원 최소 확장)

상태: 이 범위의 자체검사 결과는 `W/result.json` 에 있다. Root 공개 통합·독립 c11·정본 채택·Main/Sub 전달·성능은 미실시이며 이 절은 그 어느 것도 주장하지 않는다.

**범위.** 새 `[[bin]] campaign_read`(일반 bin 53→54), `src/campaign_read.rs`, `src/bin/campaign_read.rs`, 검사 1(`checks/campaign_read_cli.rs`)과 동결 자료 `checks/data/campaign_read/original26.json`, `Streams::print_encoded`(인코딩된 한 줄 print) 1개, T의 `Frame::pathlib_is_file/is_dir/parents_index` 상수 3개와 `push_frame` 공개(본문 불변), 런처 `terraforming_node/scripts/campaign_init.sh`, 호출부 4곳(`broad_search.sh` `--active`·`--derive cell-status`, `fetch_sub_docs.sh` `--active` 2), 격리 복사 목록 3개와 서브 배달 목록(`CAMPAIGN_TOOLS`)에 런처 추가. `campaign_init.py`(R46 바이트)는 건드리지 않았고 writer·argparse 대체는 없다.

**원본 → Rust.** `main`의 `--active`/`--derive` 분기 → `campaign_read::run`(문법 `parse_exact`, 경로 `choose_route`, 업무 `business`); `active_campaign_id`/`derive_path`/모듈 전역 루트 → 기존 T(`role_contract::campaign`)를 그대로 호출(새 규칙 없음); `print(...)` → `Streams::print_encoded`(CPython print·flush 모델, 프로필 코덱으로 한 줄 전체를 먼저 인코딩); `except (PurgeGateRefusal, WriterRefusal)` → `[campaign_init] FAIL <메시지>` + 상태 1; 처리되지 않는 예외 → Python 3.12.3 traceback 전체(소유자 프레임 표는 검토된 소유자 바이트에 단위검사로 묶임, stdlib 프레임은 `/usr/lib/python3.12/pathlib.py` 본문에 묶임) + 상태 1. 실측 원본 26(전 흐름 raw)이 정답이며 새 Rust 출력에서 기대값을 만들지 않았다. P07 의 `main` 프레임은 5122 한 줄 + 호출부터 줄 끝까지의 캐럿(5123 줄 없음), P08 은 import 시점 32행(`~`+`^`)과 pathlib 281 뿐, P10/P11 은 894/877+842 프레임이다.

**경로(업무 처리 전에만 결정, 관찰 한 줄).** `LegacyExplicit`(비어 있지 않은 `CAMPAIGN_READ_PYTHON`) · `LegacyShape`(정확한 ASCII 문법 밖) · `LegacySourceVariant`(소유자 부재·읽기 불가·다른 바이트) · `LegacyProfile`(모델링되지 않은 텍스트 프로필) — 모두 실제 `campaign_init.py` 를 `exec`(같은 PID, 자식·캡처 없음)로 실행하는 **명시된 호환 경로**이며 기본 배포의 가림막이 아니다. 네이티브 처리가 시작된 뒤의 거절·크래시·인코딩 실패·쓰기 실패는 Python 으로 재시도하지 않는다. 실행파일 부재·실행 불가는 런처가 경로 선택보다 먼저 상태 2 + 빌드 안내(명시 Python 지정이 있어도 Python 시작 없음). 관찰 파일에 쓸 수 없으면 제어 실패(상태 2)이며 업무 출력은 없다. 소유자 import 가 `IndexError` 외로 실패하는 경우는 프레임을 재현할 수 없어 제어 실패(상태 2)로 구분한다(검토된 바이트가 읽히면 도달하지 않는다).

**동결 자료.** `original26.json` 은 실제 원본 CLI 26 의 첫 종료 raw 바이트에서 비공개 픽스처 루트만 `<ROOT>` 로 바꾼 리터럴 hex 와 리터럴 픽스처 표다(추적 digest 없음). 이후 Root 의 정식 후보 바인딩은 별도의 불변 산출물로 같은 원본 픽스처·소유자 경로를 쓰므로 traceback 의 경로가 그대로 비교된다.

**검사.** `campaign_read_cli` 11: 네이티브 프로세스 24(원본 26 중 P08·P09 제외, 출력·상태·읽기 전용 FS 바이트 동일 + 관찰 한 줄 + PID), P08 함수 수준 import traceback(원본 raw), P09 선언 레거시 경로, 경로 표(명시·빈 명시·형태·변종·부재·읽기 불가·프로필, 기록 스텁 인터프리터 — 원본 프로그램이 아님: argv·cwd·stdin·환경·PID 전달), 제어 실패, 문법 표 24, 쓰기 실패 모델(상태 120·재시도 없음), 소유자·stdlib 프레임 표의 소스 결속, 런처. 영향받은 T `topology_checks` 20, B 격리 복사 단위 1, fmt, 범위 clippy `-D warnings`, 일반 릴리스 빌드(bin 54, Cargo metadata 파생) 후 실제 릴리스 ELF 를 런처로 부른 자체검사 24.

**한계(주장하지 않음).** (1) P08 의 프로세스 수준 실행은 Root 가 할당하는 5개 미만 성분의 소유 파일이 필요해 함수 수준만 확인했다. (2) 복사본 격리 자체검사(`broad_search_*`, `sweep_repeats`)는 C 안에 릴리스 ELF 가 없어 실행하지 않았다(목록만 갱신, 컴파일됨). (3) `print` 가 `derive` 에서 인코딩에 실패하거나 즉시 쓰기에 실패하는 traceback 의 프레임 모양은 원본 raw 가 없고 3.12 의 캐럿 규칙에서 추론한 것이다. (4) 호출부는 stderr 를 버리므로 런처 안내(상태 2)가 가려지고 기존 `|| echo` 대체값이 쓰일 수 있다 — Root 의 호출부 정책 결정 항목. 성능은 측정하지 않았다.

**호출부 가용성 확인(v2).** 런처에 조건부 환경 변수 `CAMPAIGN_READ_VALIDATE_ONLY=1`(값 1만, 같은 ELF·실행 비트 검사 뒤 업무 없이 0)을 두고 `broad_search.sh cell`(첫 서빙·측정·캠페인 읽기 전)과 `fetch_sub_docs.sh`(서브 접속 전)가 한 번 호출한다. stderr 를 버리지 않고 실패 상태를 전파하며, 가용성 확인일 뿐 건강 증명이 아니다. 새 argv·Rust 의미 변경 없음. 검사: `campaign_read_cli` 14(신규 3: validate-only 부재·비실행·유효, `fetch_sub_docs.sh` 원격 접근 전 중단, `broad_search.sh` 정적 순서 — 이 스크립트는 내부에서 Git 을 호출해 동적 실행은 하지 않았다).

## R48 — `manifest_contract` 명령줄 게이트 (소스 초안 · 빌드·테스트 미실행)

**범위.** T(`role-contract`)에 `manifest_gate.rs`(순수 `evaluate_contract`/`manifest_tp`/`effective_model_source`/`resolve_topology`/
`branch_topology_conflict`, 주입 가능한 `BranchProbe`)와 `manifest_gate_cli.rs`(정확한 문법·`main` 흐름·출력·JSON/인쇄 실패 traceback·`GitBranch`),
`source_identity::REVIEWED_MANIFEST_CONTRACT`; B에 새 `[[bin]] manifest_contract`(일반 bin 54→55, 개수는 Cargo metadata에서 동적으로 센다),
`src/manifest_contract.rs`, `src/bin/manifest_contract.rs`, 검사 `checks/manifest_contract_cli.rs`와 동결 자료 `checks/data/manifest_contract/`;
런처 `terraforming_node/scripts/manifest_contract.sh`; 호출부 2곳(`run_bench.sh`·`lite_bench.sh`). `manifest_contract.py`(원본 바이트)는
건드리지 않았고 프로세스 안 import 사용자 전부와 `verify_distribution` 호출은 Python으로 남는다.

**환경 이름(Rust·런처·검사가 같은 집합).** `MANIFEST_CONTRACT_COMPAT_OWNER`·`MANIFEST_CONTRACT_ROUTE_OBSERVER`·`MANIFEST_CONTRACT_PYTHON`·
`MANIFEST_CONTRACT_COMPAT_PYTHON`·`MANIFEST_CONTRACT_BIN`·`MANIFEST_CONTRACT_VALIDATE_ONLY`.

**문법 결정.** `--repo` 값이 빈 문자열이면 `argparse`가 받으므로 네이티브가 처리한다(기본 `.`). `-`로 시작하는 값·`--topology` 선택지 밖의 값·약어·`--x=y`·
중복·`-h`·`--self-test`는 업무 전에 `LegacyShape`다.

**캡처 기준.** 실제 원본 190건(Python 3.12.3 · PyYAML 6.0.1, `T/r48-original190-actual-capture`)의 stdout/stderr 바이트가 오라클이다. 계획서가 가정한
PyYAML 6.0.3은 틀렸다 — 포트(`agent_runtime_support::pyyaml`)가 6.0.1 원본 바이트와 같은지는 동결 자료가 증명해야 하며 이 초안은 증명하지 않았다.

**명시적 미해결(INCOMPLETE).** 빌드·`cargo test`·clippy 미실행. 캡처가 없는 형태는 추측 대신 드러낸다: ① 진실한 비문자열 `role`의 `effective_model_source` 키(`Unsupported`,
종료 2), ② 스스로를 포함하는 dict·JSON이 쓸 수 없는 키·4300자리 초과 정수·재귀 한계 초과의 JSON traceback(`Unsupported`), ③ `bytes` `work_dir`/`tiktoken_host_path`의
예외 문구(CPython 문서 기준 구현, 미검증), ④ 날짜/시각 `role`의 `%s`(repr 사용, 미검증), ⑤ 8 KiB 청크 경계를 넘는 잘못된 UTF-8 위치.

### R48 정제 — 컴파일 전 결정 K1·K4·K5 (소스 정제 · 빌드·테스트 미실행)

**K1 — `TERRAFORM_FLAG_GATE.C2` 호출부 단언의 배선 적응.** 사용자 기준·권위의 변경이 아니라 호출부 명령이 런처로 옮겨 간 데 따른 리터럴 갱신이다.
`claim_predicates.py`의 두 지점은 하나의 도우미 `_require_flag_gate_caller`로 모였고 요구하는 것은 (a) `run_bench.sh`가 정확한 새 명령
`bash "${MC%.py}.sh" --topology "$TOPO" --repo "$REPO" --require-flag`를 담는다 — 인자열은 같은 단언이 인-프로세스 기준 구현으로 실행한
`manifest_contract.main(["--topology", "single", "--repo", tmp, "--require-flag"])`와 같다, (b) 런처 가용성 확인
`MANIFEST_CONTRACT_VALIDATE_ONLY=1 bash "${MC%.py}.sh"`가 게이트 호출보다 앞선다, (c) 게이트 호출 바로 뒤에 `exit 4`(무효 Flag 정지)가 있다,
(d) 게이트가 작업 환경 파일(`EF=`) 적재보다 앞선다. 이전 시나리오·기대 종료 코드는 그대로이며 임의의 `bash`로 완화하거나 단언을 지우지 않았다.
인-프로세스 `manifest_contract.main` 호출은 검증/기준 평면(S7 열림)의 Python 원본이며 이 단언은 네이티브 명령줄을 증명하지 않는다 — 네이티브는 원본/네이티브
코퍼스와 새 셸 테스트가 증명한다. 쉘 문법 대응: `$MC`는 `.../manifest_contract.py` 경로이고 `${MC%.py}`는 그 접미사 `.py`를 떼어 같은 폴더의 런처
`manifest_contract.sh`를 가리킨다.

**K4 — 업무 전 프로필 경로(R47식) 유지.** `TextProfile::from_environment`가 모델링하는 것만 `NativeCurrentSource`다: `LC_ALL`/`LC_CTYPE`/`LANG`이 UTF-8
문자셋이거나 `C`/`POSIX`(UTF-8 모드·로캘 강제 포함), `PYTHONUTF8`은 0/1, `PYTHONIOENCODING`은 `utf-8`·`ascii`(핸들러 `strict`/`surrogateescape`/
`backslashreplace`). 그 밖 — 예컨대 E04의 `PYTHONIOENCODING=latin-1`, UTF-8도 C도 아닌 로캘 문자셋, 알 수 없는 핸들러, 잘못된 `PYTHONUTF8` — 은 계획된
`LegacyProfile`이며 현 모델이 의미 전부를 모델링하지 않으므로 경로 명세를 조용히 바꾸지 않았다. 빈 `--repo ""`(P12)는 원본 cwd 의미로 유효하므로
네이티브가 처리한다: 이는 소스 네이티브 커버리지 확대(내부)이지 외부 동작 변경이 아니다(원본은 `os.path.abspath("")` = cwd). 등호형·중복·약어 플래그는
선언대로 `LegacyShape`이다.

**K5 — 원본 소스에서 읽히는 의미 격차를 구현(미설계 `Unsupported` 제거).**
- `role` 키(소유자 236-239행 사전 컴프리헨션): 거짓 값(`None`/`""`/`0`/`false`/빈 컨테이너)은 `node<i>`(i는 모든 노드를 센다); 문자열은 그대로;
  `int`/`float`/`bool`은 Python 키 동치(`1`·`1.0`·`True`는 하나, 첫 키 객체·위치 유지, 마지막 값)로 `PyDict::insert_key`와 YAML이 쓰던 `dict_key`
  (공개로 노출)에 위임; 날짜/타임스탬프/바이트는 유효한 키(게이트는 성공, `json.dumps`만 `TypeError: keys must be str, int, float, bool or None, not date`);
  진실 값인 list/dict/set는 `TypeError: unhashable type: 'list'` 등 — 3.12는 컴프리헨션을 인라인하므로 `<dictcomp>` 프레임 없이 `evaluate_contract`
  237행 한 프레임이며 강조 구간은 미확정(전체 행·캐럿 없음으로 가정, 캡처 X03/X04가 확정).
- `%s` 서식: `str(date)`=ISO 날짜, `str(datetime)`=공백 구분 `isoformat`(`py_str`); 나머지는 `repr`. 메시지의 role·work_dir·플레인 줄에 적용.
- JSON 실패 traceback: 사전 키 거부(`encoder.py` 377행)와 사전 순환(341행)을 3.12 인코더 소스에서 읽어 구현 — 둘 다 **원본 캡처 전 미검증**.
  4300자리를 넘는 정수의 `%s`/JSON은 계속 명시 `Unsupported`(종료 2, 숨은 대체 없음)이며 이는 최종 대체가 아니라 열린 INCOMPLETE다.
- 여전히 열림: 8 KiB 청크 경계 뒤의 잘못된 UTF-8 위치, 4300자리 정수 문구·프레임, `bytes` `work_dir`/`tiktoken_host_path`의 예외 문구, 깊은 재귀.
  리터럴 근거는 `extra-original-proposal`(X01–X14, 데이터만)이 후속 캡처로 제공한다.

### R48 빌드·검사 결과 (비공식 사본 · 원본 190 + 추가 14 동결 raw로 교정)

**상태: INCOMPLETE** — 컴파일·clippy·단위/골든/셸 검사는 통과했으나 아래 "열림" 항목과 C2 전체 시나리오(샌드박스 `/tmp` 쓰기 제약), 196 코호트·배포·성능은 NOT_RUN이다.

**원본 raw로 바로잡은 가정(프레임 투영 없이 실제 바이트에서).**
- X03/X04 `TypeError: unhashable type` 프레임의 강조 구간은 키-값 문장에서 첫 글자(여는 괄호)를 뺀 전체(67자)다 → `EFFECTIVE_KEY.expr`.
- 날짜 `role`은 `nodes` 목록에서 **값**으로 먼저 출력되므로 `--json`은 `Object of type date is not JSON serializable`(캡처 X06)로 실패한다 — 키 거부(377행)는 이 경로에서 도달하지 않는다. 키 거부(`custom_model_paths`의 날짜 키)와 리스트 안 큰 정수(314행) 프레임은 **캡처가 없어 미검증**이다.
- 사전 순환(X10)은 프레임 432·406·406·341, 큰 정수 JSON(X12)은 432·395(`yield _intstr(value)`, 강조 `_intstr(value)`), 큰 정수 평문(X11)은 454행 `~~~^^^…`이며 메시지는 `Exceeds the limit (4300 digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit`(자리수 문구 없음).
- UTF-8 위치(X13/X14): 8 KiB 청크 상대 위치이며 청크 경계에 걸친 글자의 앞 바이트가 다음 디코드 입력에 이어져 위치에 포함된다 — 포트가 이미 그렇게 읽는다(수정 없이 일치).
- `date`/`datetime` `%s`와 순환 사전의 가정은 캡처와 일치했다.

**검사 범위.** T `manifest_gate_checks`(12) · B `manifest_contract_cli`(8: 문법·경로·소유자 프레임 소스 결속·원본 190 골든 재생·추가 14 골든 재생·구조 검사) ·
B `manifest_contract_shell`(5: 런처 부재/비실행 exit 2가 어떤 레거시보다 먼저·`VALIDATE_ONLY` 가용성만(옵저버·시작 없음)·소유자 export와 빈 인터프리터 해제·
호출부 안내 전파·네이티브 Flag 미발급 4/발급 시 게이트 통과 후 환경 파일 부재 2/소유자 부재 분기 4). 골든은 비공개 경로 접두(`<FIXROOT>`/`<OWNER>`)만 치환한 리터럴
바이트이며 기대값·지원 범위를 편집해 실패를 숨기지 않았다. 테스트는 정본 Python 소유자를 실행하지 않는다(동일 바이트 소유자는 네이티브로 라우팅).

**K1 시나리오 적응(Root 승인 범위).** C2의 hermetic `run_bench.sh` 시나리오는 게이트 *답*을 가짜 python3로 대신했으므로, 같은 자리에 게이트 런처 대역
(`manifest_contract.sh`: 가용성·게이트 모두 성공)을 둔다. 실제 런처·네이티브 게이트는 셸 검사와 원본/네이티브 코퍼스가 증명한다.

**열림.** `bytes` `work_dir`/`tiktoken_host_path` 예외 문구(캡처 없음) · 키 거부(377)/리스트 큰 정수(314) 프레임 · C2 시나리오의 `vllm bench serve` 이후 구간(샌드박스가
`/tmp` 쓰기를 막음 — 게이트 통과까지만 확인) · 196/추가 14 네이티브 코호트 · 배포·성능.

### R48 부록 — 추가 9 원본(Y01~Y09) 통합 · 이전 주장 철회

**철회.** 앞 절들의 "4300자리 정수 `role` 키는 사실상 도달 불가" 및 `bytes` `work_dir`/`tiktoken_host_path`가 열림이라는 서술 중 앞의 것은 소스가 뒷받침하는 제외가 아니었다.
실제 캡처 Y08(평문 exit 0)·Y09(`--json` ValueError exit 1)가 둘 다 도달함을 보였다. 이 부록이 앞 절을 대체한다(이전 결과 파일은 고치지 않았다).

**실제 raw로 확정된 것(원본 Python 3.12.3 / PyYAML 6.0.1, 한 번 실행).**
- Y01 `custom_model_paths`의 날짜 키: 프레임 432·406(res)·377(`_iterencode_dict`, `raise TypeError(f'keys must be …` 첫 줄, 캐럿 없음), `keys must be str, int, float, bool or None, not date` — 소스에서 읽은 프레임과 일치.
- Y02 리스트 안 큰 정수: 432·406·314(`yield buf + _intstr(value)`, 캐럿 `_intstr(value)`) — 일치. Y09 노드 안 큰 정수: 432·406·326·395 — 일치(역할은 JSON 키가 아니라 `nodes` 값으로 먼저 실패).
- Y03/Y04 bytes: `a bytes-like object is required, not 'str'`(genexpr 캐럿 `w.rstrip("/")`) / `startswith first arg must be bytes or a tuple of bytes, not str`(캐럿 `tik.startswith(w.rstrip("/") + "/")`) — 일치. Y05 같은 bytes는 평문 exit 0, Y06은 `Object of type bytes is not JSON serializable` — 일치. 이로써 `bytes` 한계의 "열림"이 닫혔다.
- Y07 NaN 역할: `effective_model_source`는 **키 3개**(`.nan` 공유 객체 → custom, 앵커+별칭 → ephemeral, 별개 `!!float nan` → managed)다. 표준 숫자 `nan` 동치가 아니라 Python 객체 동일성이다.

**소스 변경(실제 raw에서 연역된 유일한 격차).** 파일 로더는 NaN 객체를 만들지 않아(`.nan`·별칭이 모두 같은 맨 실수) 역할 키가 5개가 된다. 그래서 (1) 공통 패키지에
`pyyaml::safe_load_objects`(NaN 객체 선택만 다른 같은 로더)를, (2) T에 `load::load_yaml_objects`를 추가하고 `manifest_gate_cli`가 그것을 쓰며, (3) 게이트의 역할 키 정체성
(`nan_origin`)은 객체의 할당 주소다. 다른 소비자의 `safe_load`/`load_yaml`은 바뀌지 않았다. **빌드·테스트는 이 호출에서 실행하지 않았다.**

### R48 부록 2 — extra9 소스 통합의 빌드·검사 결과

**범위 충족(비공식 사본 · 사적).** 원본 213 형상(190+14+9)에 대한 `manifest_contract` CLI/라이브러리 소스: 잠금·오프라인 컴파일, `cargo fmt --check`(Q/T/B 모두 0),
`clippy --all-targets -D warnings`(Q/T/B 모두 0), 단위·골든·셸 검사 통과. 전체 작업·196+14+9 네이티브·배포·성능·남은 Python 임포터·S3~S7은 **열림/NOT_RUN**이다.

**명령(환경 먼저).** `TC=<관리 1.99.1 bin>` 을 PATH 맨 앞에, `RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=2 CARGO_HOME=<B>/cargo-home CARGO_TARGET_DIR=<B>/cargo-target`.
각 매니페스트에서 `cargo fmt --check` · `cargo clippy --offline --locked --all-targets -- -D warnings` · Q `cargo test --offline --locked`(86 = lib 80 + pycall 6; 두 개는 신규
`safe_load_objects` 검사) · T `cargo test --offline --locked`(65: lib 1 · identity 21 · manifest_gate 13 · role_contract 10 · topology 20) · B
`cargo test --offline --locked --test manifest_contract_cli --test manifest_contract_shell`(14 = 9 + 5) · 다른 두 소비자 `cargo check --offline --locked --all-targets`(0/0) ·
B `cargo build --offline --locked --release --bins`(0). 대상 수는 `cargo metadata`에서 읽었다: B lib 1 · bin **55** · test 85 · build script 1, Q lib 1 · test 1, T lib 1 · test 4, 기능 없음.

**골든.** `original190.json` = 네이티브 172 + 레거시 경로 단언 18, `extra14.json` = 네이티브 14, `extra9.json` = 네이티브 9. 기대 바이트는 문자 그대로(경로 접두 `<FIXROOT>`/`<OWNER>`만 치환)이며
이번 창에서 쓰지 않았다. 네이티브 성공 또는 치명 1 형상에 대한 `Unsupported` 종료 2는 없다. Y07의 NaN 객체 동일성 3키(custom/ephemeral/managed)는 라이브러리 재생과, **새 ELF를 자체
임시 픽스처 사본 9개에 돌려** 첫 종료·stdout·stderr 바이트가 골든과 같음으로 관찰했다.

**재사용(재실행 안 함).** `load_yaml`은 같은 `safe_load`를 그대로 쓰고(함수 포인터 간접화) T 전체와 B 라이브러리 컴파일이 통과했으므로 `campaign_read_cli`·`bench_shell_*`·
classify_cell 계열은 이전 통과를 재사용한다. C2 독립 리뷰 PASS, L1~L10·R/P/S 자체 시험, render_sub_env 23, antipattern T1~T15, 간선 검사는 이전 소스에서 유효하며 이번 변경과
접점이 없다. 닫힌 R43/R44/R45/R47 코호트와 원본 213 실제 코호트는 재실행하지 않았다.

**ELF.** 새 `manifest_contract` release sha256 `67d2a3b95bb90b49984507d1af637b49d2926ab2a794af140c127e66b97cca63`(이 소스의 현재 산출물). 이전 `91c29104…`는 extra9 이전 소스의 옛 증명용이며 현재가 아니다.

### R48 부록 3 — gap3 원본(R48K01~K03) 반영: 소스 수정(빌드·검사 미실행)

**원본 raw로 확정된 것(Python 3.12.3 / PyYAML 6.0.1, 한 번 실행; K01은 의도한 분기에 닿지 않았다).**
- R48K01: 흐름 매핑 안의 4335자리 정수 키는 YAML 스캐너의 단순 키 한계(1024자, `scanner.py:279-293`)에 걸려 키가 사라지고 `:`만 남아 **exit 2 파서 오류**(`while parsing a flow mapping … expected ',' or '}', but got ':'`)다. 이 관찰이 원본 계약이며 K01 소스 가설(JSON ValueError)은 불일치로 남긴다(수정·재생·추측 프레임 없음). 네이티브 로더(공통 `pyyaml` 포트)는 같은 한계를 이미 구현하므로(`scanner.rs` `1024`) 로더 수정은 없고, 골든 재생이 증명한다.
- R48K02: 깊이 1003의 지연 앵커 사슬은 YAML 단계에서 실패하지 않고 JSON에서 `RecursionError: maximum recursion depth exceeded`로 끝난다. 프레임: 432 · `_iterencode_dict` 406 ×(3 출력 + `[Previous line repeated 991 more times]`) · 마지막 403 `chunks = _iterencode_dict(value, _current_indent_level)`(캐럿 호출식). 규칙: 인코더는 모듈·`main`·`dumps`·`encode`·`_iterencode`의 5 프레임 아래에 컨테이너당 1 프레임을 쌓고 한계 1000에서 다음 컨테이너 생성이 실패한다 → 컨테이너 995개까지 허용(공통 포트의 `PRETTY_MAX_DEPTH`와 같은 값).
- R48K03: 삭제된 cwd에서 상대 `--repo`는 `os.getcwd()`가 실패한다: 프레임 `<module>` 462 · `main` 414(`repo = os.path.abspath(args.repo)`, 캐럿 호출식) · `File "<frozen posixpath>", line 420, in abspath`(**소스 줄·캐럿 없음**), `FileNotFoundError: [Errno 2] No such file or directory`. 실제 `/usr/lib/python3.12/posixpath.py` 프레임은 출력되지 않는다.

**소스 수정(`manifest_gate_cli.rs`, 사건·경로·고정 출력 표 없음).**
1. `render_traceback`이 `traceback.StackSummary.format`처럼 같은 (파일·줄·함수)의 연속 프레임을 3개까지 출력하고 이어서 `[Previous line repeated N more time(s)]`를 쓴다.
2. 재귀 한계: 경로 위 첫 컨테이너 깊이 995에서 `Found::TooDeep(자식 종류)`를 찾고, 조상 컨테이너마다 `yield from chunks` 프레임 + 실패 컨테이너의 자식 생성 줄(부모/자식 종류별 401/403/405, 321/323/325)을 쓴다. 사전-in-사전은 캡처로 확정(K02), 나머지 조합은 인코더 소스에서 읽었고 **캡처 전 미검증**이다.
3. 삭제된 cwd: 상대 경로일 때만 `current_dir()` 실패를 `os.path.abspath`의 `OSError`(errno별 하위 클래스: ENOENT→FileNotFoundError, EPERM/EACCES→PermissionError, 그 외 OSError)로 올리고 `<frozen posixpath>` 프레임으로 끝낸다 — 더 이상 `unwrap_or_default()`로 삼키지 않는다. 절대 경로는 cwd를 묻지 않는다.
4. 키 위치의 4300자리 초과 정수: 덤퍼는 정수 키를 자릿수 그대로 쓰므로(성공으로 보임) 성공 경로에서도 인코더 순서의 첫 실패가 정수 키인지 확인하고 `_iterencode_dict` 373 `key = _intstr(key)`(캐럿 `_intstr(key)`) 프레임으로 `ValueError`를 낸다. **이 줄은 소스에서 읽은 설계이며 실제 raw(R48K04)가 올 때까지 구현 완료로 주장하지 않는다.**
5. F2 낡은 주석(순환 사전·키 거부가 "캡처 전 미검증")을 고쳤다 — X10(순환)과 Y01(키 거부)은 캡처됐고 골든이 재생한다.

**데이터·검사(소스만; 실행은 다음 빌드 창).** `checks/data/manifest_contract/gap3.json`(3행, 실제 raw 문자 그대로 · `<FIXROOT>`/`<OWNER>`만 치환 · K03는 `deleted_cwd`) + 정확한 Root 픽스처 3개; 재생 하네스가 K03에서 자체 임시 디렉터리를 만들고 지운 뒤 실행한다. 신규 테스트: `gap3_golden_rows_replay_natively`, 반복 프레임 축약 단위, 리스트 사슬 재귀 한계(소스 유도), 명시 블록 키의 4300자리 정수 키(소스 유도).

**영향 지도.** 닫힌 Native219 증거는 ELF 67d2…·소스 홀드 44af…에 묶인 증거로 그대로 유효하다(재실행하지 않는다). 새 ELF에서 행동이 접하는 곳만 다시 증명한다: ① 모든 크래시 출력이 지나는 `render_traceback`(213 골든 어디에도 3개 초과 연속 동일 프레임은 없다 → 바이트 불변이어야 함), ② JSON 실패 행(YT16~21·X06·X10·X12·Y01·Y02·Y06·Y09: 새 `find_failure` 분기가 같은 결과를 내야 함), ③ 성공 JSON 경로의 정수 키 확인(키가 큰 정수가 아니면 불변), ④ 상대 `--repo` 행(cwd가 있으면 불변). 공통 포트(Q)와 T의 게이트/로더는 바뀌지 않았으므로 Q·T 단위 시험의 이전 통과는 재사용하고, T는 컴파일만 영향 받는다. 필요한 새 검사: `manifest_contract_cli`(골든 190+14+9+3 재생 + 구조 시험) · `manifest_contract_shell`(링크만 다시) · clippy/fmt.

**열림.** R48K04(진짜 키 분기)는 raw 전까지 미구현 주장 없음, 프로세스 안 Python 임포터·S3~S7·배포·성능·196+14+9+3 네이티브 재결합은 열림.

### R48 부록 4 — R48K04 실제 raw 반영(키 위치의 4300자리 정수)

실제 raw(PID 442711, exit 1, stdout 0; 독립 감사 PASS)의 프레임은 owner 462/450 · `json/__init__.py` 238 · `encoder.py` 202/432/406 · **373 `key = _intstr(key)`(캐럿 `_intstr(key)`)**,
`ValueError: Exceeds the limit (4300 digits) …`였다. 소스에서 설계해 둔 키 분기(조상 `yield from chunks` 프레임 1개 뒤 373)와 **바이트까지 같다** — 코드 변경 없음, 부록 3의 "미검증" 표기가 해소됐다
(남은 미검증: 재귀 한계에서 사전-in-사전이 아닌 컨테이너 조합). `key4.json` 골든 1행 + 정확한 Root 픽스처 `R48K04.yaml`을 추가했고 이전 골든(190·14·9·gap3)은 바이트 불변이다.
영향 지도 정정: 부록 3은 T에서 `manifest_gate_cli.rs`만 바뀐 것처럼 적었으나 `manifest_gate.rs`도 바뀌었다(머리 주석만, 동작 불변). 따라서 "T 게이트/식별/위상/역할 단위 시험의 이전 통과 재사용"은
**경로 의미 한정**(동작이 같은 경로의 이전 통과만)이며 시험은 실행한 것으로 세지 않는다 — 새 빌드 창에서 T 전체 컴파일·`manifest_gate_checks`·`manifest_contract_cli`(골든 190+14+9+3+1)를 다시 증명한다. Q는 바뀌지 않았다.


## R49 — `turn_budget.py` 를 공개 CLI `turn_budget` + 비공개 브리지로 (T 순수 모듈 · 얇은 Python 어댑터)

상태: **소스 작성만 했다.** Cargo 빌드·테스트·clippy 와 실제 실행은 미실시(`rustfmt` 구문·서식 검사만 통과). 이 절은 통합·독립 검증·정본 채택·Main/Sub 전달·성능 어느 것도 주장하지 않는다. 원본 139 raw 는 닫힌 참조이며 재수행하지 않았다.

**범위.** T: `.claude/rust/role-contract/src/turn_budget.rs`(순수 업무: `schema_cap`·`declare`·`--caps` 문서·자체검사·프레임 표) + `lib.rs` 한 줄. B: 새 `[[bin]] turn_budget`(일반 bin +1), `src/turn_budget_cli.rs`(문법·경로·`json.dump` 청크 출력·traceback·종료 상태), `src/turn_budget_bridge.rs`(import 브리지), `src/bin/turn_budget.rs`, 생성 자료 `src/help_layouts_turn_budget.rs`, `argv.rs` 의 `TURN_BUDGET` Spec, `cli.rs` 의 `Streams::write_encoded`(미리 인코딩된 한 번의 `write`), 검사 1(`checks/turn_budget_original_cases.rs`)과 동결 자료 `checks/data/turn_budget/original139.json`. 런처 `terraforming_node/scripts/turn_budget.sh`, 어댑터 `turn_budget.py`.

**원본 → Rust.** `schema_cap`/`declare`/`_self_test`/`--caps` → `role_contract::turn_budget`(Python 값 의미를 `PyValue` 로 보존: `caps or {...}` 진리값, bool 은 int, `v > caps[k]` 의 TypeError/KeyError, 검증 순서 unit→timeout→max_turns→source, 호출마다 스키마 재독·캐시 ✗). `main`(argparse) → `turn_budget_cli::run`. 소유자 경로는 런처/어댑터가 `TURN_BUDGET_OWNER_PATH` 로 내보내며 REPO 는 그 어휘적 위 네 단계(심볼릭 링크 해소 ✗; relay 의 `--repo-root` 는 기본 스키마와 무관).

**경로.** 소유자 경로 미설정·모델링되지 않은 텍스트 프로필은 업무 전에 상태 2 의 **보이는 제어 실패**다(Python 경로 없음). stdout 코덱이 help 를 쓰지 못하면 그것은 원본의 `UnicodeEncodeError`(argparse 프레임 · 첫 종료 1)이며, exit-120 안내는 실제 코덱 이름(`encoding='ascii'` 등)을 쓴다(`Streams::finish_with_codec`; 다른 호출자의 `finish` 는 그대로). `TURN_BUDGET_ROUTE_OBSERVER=<파일>` 이 CLI 와 비공개 브리지 모두 실행마다 `start`(업무 전)·`end`(결과) 두 줄을 남긴다(실제 Rust pid·ppid·선언된 소유자 경로·route·shape·outcome; 브리지의 `raised:<클래스>` 는 전송 종료 0 이 실어 나른 예외이고 import 측이 관측하는 것은 어댑터가 일으키는 예외다). 업무 시작 뒤 Python 재시도 없음. 실행파일 없음은 런처가 상태 2 로 먼저 실패한다.

**브리지 `turn-budget-bridge/1`.** `TURN_BUDGET_BRIDGE` 환경이 정해진 같은 실행파일(공개 인자 추가 ✗). 요청·응답은 타입 태그 JSON(정수=부호+16진 → 4300 자리 제한 없음, bool 을 int 보다 먼저, `pathlike`=`fspath`+`str()`). 응답은 `returned`/`raised`(`cls`·`ctor`·`ctor_kwargs`·`context`·`suppress_context`·진단용 `message`)/`unsupported`.

**프레임.** CLI 전체 traceback 은 관측 프레임(C07·C09~C12·C15)으로 고정했고, 관측 밖의 stdlib 줄(`json/__init__.py` 335, `json/decoder.py` 340·353)과 `_self_test` 의 후반 print 위치는 3.12.3 소스 줄에서 **도출**했다(관측 아님 · 이 표기를 유지한다). 자체검사의 `revived` 목록은 모듈 이름 **사실**에서 Rust 가 블랙리스트·순서·라벨로 판정한다: import 측 `_self_test()`/`main()` 은 자기 모듈 전역 이름만 `TURN_BUDGET_MODULE_NAMES` 로 전달하고, 독립 실행은 이 프로그램 자신의 공개 이름(`STANDALONE_GLOBALS`)을 쓴다(Python 실행을 흉내 내지 않는다).

**검사(작성만).** `turn_budget_original_cases` 4: 명령줄 39 중 별도 파이프와 stdout=/dev/full(120) 행을 네이티브 프로세스로 바이트 비교, API 100 중 Python 서명 오류 3행을 뺀 97행을 브리지로(분류·`str()`·값), 소유자 신원 없는 실행의 보이는 실패, 어댑터의 등급표·업무 조각 부재. 병합 파이프(C16)·PTY(C17)·닫힌 fd(C38)는 동결 바이트만 싣고 설정은 이후 단계다. 예외 `args`/속성은 Python 어댑터가 `ctor` 로 재구성하므로 Rust 검사가 단언하지 않는다.

**잔여(숨기지 않음).** (1) 세 importer(`relay`·`bootstrap_canary`·`selftest_harness_switch`)와 어댑터는 Python 으로 남는다. (2) U2: in-process 예외의 traceback 바이트(원본 소스 프레임)는 재현하지 않는다 — 열림. (3) U3: 어댑터는 정확한 기본 타입·문자열 키 dict·`os.PathLike` 만 전달하고 그 밖은 업무 전에 `NotImplementedError` 로 거절하며(int 경로=fd 포함), 사용자 정의 `__bool__`/`__eq__` 객체의 원본 동작은 재현하지 않는다. (4) 이 변경은 `help_layouts` 생성기(`gen_help_layouts_turn_budget.py`, 저장소 밖 · COLUMNS 1~700 재생)로 만든 데이터이며 COLUMNS 80·40·PTY 80 의 저장 raw 와 대조했다. (5) 컴파일되지 않았으므로 타입 오류가 남아 있을 수 있다. 개수: C49(B54)+1 = 55; 통합 뒤 개수는 cargo metadata 로 도출한다(고정값 ✗).

**계약 보정(후속).** (R5) 자체검사 tripwire 는 위처럼 모듈 이름 사실로 판정. (R6) ASCII 등 stdout 코덱의 `--help` 는 원본의 `UnicodeEncodeError` + argparse 프레임(`parse_args`→`parse_known_args`→`_parse_known_args`→`consume_optional`→`take_action`→`_HelpAction.__call__`→`print_help`→`_print_message`)이며 이 프레임 줄은 호스트 argparse 3.12 소스에서 **도출**했다(동결 139 에 이 행이 없다 → 데이터 전용 제안 행 R49C40). 코덱이 틀린 exit-120 안내 헤더는 관측 없이 도출한 상태다. (R7) `json.dump` 값 오류는 이 CLI 의 실제 형태에서 도달 불가다: `--caps` 문서는 `json.load` 가 읽은(4300 자리 이하 정수 — `str()` 도 같은 한도라 출력 가능 — 또는 bool) 두 값과 고정 문자열이고, `declare` 결과는 argparse `int()`(같은 4300 자리 한도)/None 과 문자열이며 모두 깊이 1 이라 `ValueError`·`RecursionError` 가 날 수 없다. 브리지는 정수를 16진으로만 실어 십진 한도를 만들지 않고, 원본 f-string 한도는 도달 가능한 곳에서 그대로 발생한다. (경로 관측) CLI 는 직접 ELF, API 100 은 import 측 Python + 도달 시 Rust 자식(Python 서명 오류 D57–D59 는 Rust 호출 없음)이며 실제 개수는 실행에서 센다.


## R48 + R49 — 합친 소스 에포크 (A379 기준 · 빌드·테스트 미실행)

상태: **소스 병합만 했다.** Cargo 빌드·테스트·clippy 와 실제 Native 실행은 미실시(`rustfmt` 서식·구문 검사만). 두 변경(R48 `manifest_contract` 게이트 · R49 `turn_budget` CLI+브리지)은 A379 를 공통 기준으로 한 세 갈래 병합이며, 겹치는 파일(`role-contract/src/lib.rs`·B `Cargo.toml`·B `src/lib.rs`·`SKILL.md`·`rust_runtime.md`·이 문서)은 양쪽 변경을 모두 보존했고 R49 만 바꾼 파일(`argv.rs`·`cli.rs`·`antipattern_scan.py`·`turn_budget.py`)은 R48 쪽이 A379 와 같아 R49 바이트를 그대로 썼다. 새 R49 파일은 동결 바이트 그대로다.

**개수.** 일반 bin = A379 54 + `manifest_contract` + `turn_budget` = **56** 로 기대하며(Cargo.toml 파싱으로 확인한 정적 수), 고정값으로 쓰지 않고 빌드 창에서 `cargo metadata` 로 도출한다. 테스트 타깃 B 86 · T 4(`manifest_gate_checks` 포함).

**K05(G4-F1).** 직렬화할 수 없는 잎(바이트·불투명)은 컨테이너보다 한 프레임 깊다 — 994 겹 컨테이너 아래의 바이트 잎은 `_default(o)` 호출(line 439)에서 RecursionError(실제 raw R48K05 로 확정 · golden `bytedepth.json`), 993 은 TypeError, 995 는 컨테이너 자식 생성 줄(소스 도출·raw 없음). 게이트 문서의 "모든 형태가 raw 로 덮였다" 표현은 **유한한 관측 분기(원본 218)** 와 알려진 한계로 고쳤다. 알려진 한계: 가장 깊은 수준의 float 잎(`floatstr` 프레임)과 995 겹의 순환 불투명 값은 모델링하지 않았다.

**C40(R6).** 실제 R49C40(`--help` · `PYTHONIOENCODING=ascii`)이 도착했다: 프레임 1908/1941/2162/2102/2017/1156/2648/2654 와 소유자 162/179, `UnicodeEncodeError: 'ascii' codec can't encode character '\ud134' in position 173`. 도출했던 프레임 표와 위치(저장된 COLUMNS 80 배치의 첫 비ASCII 문자 위치 173)가 일치한다. 도출 때 쓴 자체 검사의 문구("characters in position")는 실제와 달라 **복수형 → 단수형 문자 표기**로 고쳤고, golden `turn_budget/c40.json`+검사를 추가했다(실행 전). 이 한 분기만 관측이며 다른 코덱·폭 전부가 아니다. C16/C17/C38(병합 파이프·PTY·닫힌 fd)은 여전히 동결 바이트만 있고 설정은 이후 단계다.

**잔여.** U2(in-process 예외의 traceback 바이트) · U3(지원하지 않는 Python 객체) · 세 importer 는 Python 어댑터 · `verify_distribution` 의 `turn_budget.py --self-test` 는 빌드된 런타임을 요구한다 · 비용·성능·배포·S3 전체는 열림.

## R48 — in-process API (S1–S6) 와 deep2 (K06·K07) — 소스 + 자체 검사 (실제 빌드·테스트 실행됨)

**상태: 부분 완료(INCOMPLETE).** 구현·자체 검사는 아래이며, Root 소유 미래 실행(candidate API87 · 공개 CLI 7 · R49 candidate140 · 쌍 캡처)과 U2/U3는 열려 있다.

**구조.** T `manifest_gate.rs`: topology/explicit 이 임의 값(`evaluate_contract_value`·`manifest_tp` 가 `&PyValue`), 예외에 `name`/`obj` 경로(`Step`), `manifest_gate_api.rs`(결과 **계획** · `manifest_tp_answer`·`effective_model_source_which`·`resolve_topology_answer`·`load_manifest_answer`). Q: YAML 순환은 `Typed::Cycle(up)` 마커(조상까지 컨테이너 단계 수; `!!omap` 항목은 튜플이라 +2)이고 공유 노드 재사용은 상대 거리가 같을 때만. B: `manifest_contract` 실행파일에 비공개 브리지 모드(`MANIFEST_CONTRACT_BRIDGE`, 새 bin 없음 · 56 유지) + `typed_wire.rs`(관측기와 같은 타입 그래프 wire: 별칭·순환은 `ref`). Python: `manifest_contract.py` 는 얇은 import 어댑터(원본 `__doc__`·서명·상수; `VALID_MODES` 리터럴; `_load_manifest` 는 호출 시점 `import yaml`), 원본 바이트(e84fa13c…)는 `manifest_contract_compat_owner.py` 에 그대로(사전 분류된 레거시 경로 전용 · `REVIEWED_MANIFEST_CONTRACT` 가 이 파일에 묶임 · CLI 레거시 경로는 공개 이름으로 실행해 usage 가 같다).

**점검(출처).** 87행 golden(`checks/data/manifest_contract_api/api87.json` = 수락된 raw 의 literal 복사)을 브리지에 통과시켜 raw 와 비교(81 값/예외 · 4 main 분류 · 2 정적). 정적 어댑터 검사(`ast` 만)와 합성 입력 smoke 는 B48 `api-impl-checks/`. 해석기 PyYAML 은 **실제 6.0.3**(이전 6.0.1 표기는 UNVERIFIED; c11 이 핵심 순수 Python 모듈 16개가 6.0.1/6.0.3 바이트 동일(`__init__` 버전 줄만 다름)임을 대조 → 프레임 지도는 반박되지 않음). 옛 raw·golden·provenance 는 다시 쓰지 않았다.

**deep2.** 실제 R48K06(995 겹 아래 float 값 → 컨테이너 자신의 `floatstr` 호출, 줄 398)과 R48K07(995 겹의 순환 dict → 자식 generator 생성 호출, 줄 403, circular 검사 이전)이 측정되었다: Q 덤퍼가 float·순환 마커에도 프레임 한도를 적용하고 T 가 float 행(398/317)을 갖는다. 알려진 한계(raw 없음): float dict KEY(364) · set 안의 float · list 안의 float 은 소스 도출만(317).

**열림(숨기지 않음).** U2(in-process traceback 프레임은 어댑터의 것) · U3(지원 타입 집합 밖 인자는 업무 전 `NotImplementedError`) · set 멤버 순서는 정규화 안 함 · `Mark` 객체 동일성(PyYAML 이 context/problem mark 에 같은 객체를 쓰는 경우)은 실측 87행 밖이라 보존 안 함 · 모듈 사실: 원본의 `argparse` 이름은 어댑터에 없고 비공개 구현 이름이 더 있다(기록, 가림 ✗).


## R48 — v4 소스 리뷰 교정 B1~B5 + K08 (소스 + 자체 검사 · 비공식)

**B1 git 자식 환경.** 어댑터·런처가 바꾸는 배관 변수만 호출자의 원래 값으로 되돌린다. 기록 대상은 정확히 여섯 이름(`MANIFEST_CONTRACT_{BRIDGE,BRIDGE_REQUEST,COMPAT_OWNER,PUBLIC_ARGV0,PYTHON,VALIDATE_ONLY}`)이고, 값(또는 미설정)은 `MANIFEST_CONTRACT_CALLER_<이름>` 운반 변수 + 표지 `MANIFEST_CONTRACT_CALLER_ENV` 로 건너간다. 네이티브 `ChildEnv`(role-contract `manifest_gate_cli`)가 `git` 프로브와 레거시 인터프리터 exec 에 같은 계획을 적용한다: 표지가 없으면 프로세스 환경이 곧 호출자의 것이므로 아무것도 바꾸지 않는다. 호출자의 다른 변수(자격 증명 포함)는 읽지도 복사하지도 직렬화하지도 않는다. 예약 이름 한계: 호출자가 `MANIFEST_CONTRACT_CALLER_*` 를 이미 갖고 있으면 그 값은 보존되지 않는다(런처의 비공개 프로토콜 이름).

**B2 VALIDATE_ONLY.** 비공개 브리지(`MANIFEST_CONTRACT_BRIDGE` 설정)에서는 런처가 가용성 검사(ELF·실행 비트)만 하고 곧바로 exec 한다 — `MANIFEST_CONTRACT_VALIDATE_ONLY=1` 의 `exit 0` 은 CLI 가용성 검사일 뿐 브리지의 일이 아니다. 호출자의 값은 `git` 자식에게 그대로 간다. CLI 동작은 불변(자체 검사: VALIDATE_ONLY=1 이면 여전히 exit 0, stdout·stderr 빔). 폴백 없음.

**요청 fd 누수.** 요청은 mode 0600 개인 임시 파일(`MANIFEST_CONTRACT_BRIDGE_REQUEST`)로 건너가고 답 직후 삭제된다. 브리지 프로세스에 요청용 추가 descriptor 가 없으므로 `git` 자식은 호출자의 0~2 외에 아무것도 상속하지 않는다(원본은 `close_fds` 였다). 호출자의 stdin 은 RPC 로 대체되지 않는다. 임시 파일에는 호출자가 넘긴 객체의 직렬화가 잠시 남는다(mode 0600, TMPDIR 아래, 답 직후 `unlink`).

**닫힌 fd 0 (열림).** 호출자의 fd 0 이 닫혀 있으면 Rust 런타임이 main 전에 0~2 를 `/dev/null` 로 다시 연다. 가짜 git 이 보는 fd 0 은 원본 대조(호출자 직접 실행)가 "닫힘"인 반면 이 경로는 `/dev/null` 이다(자체 검사 `closed_fd0_observed_in_git_child`). 진짜 `git` 은 자기 시작 시 닫힌 표준 descriptor 를 `/dev/null` 로 정리하므로 관측 차이는 `git` 이름의 비-git 실행파일에서만 보인다. 동등을 주장하지 않는다.

**B3 정책 바인딩.** `MODEL_ACQUISITION_TERNARY_GATE.C1`, `TERRAFORM_FLAG_GATE.C1/C3` 의 집행 근거를 어댑터 토큰에서 실제 집행자로 옮겼다: `manifest_gate.rs`(`VALID_MODES`·`effective_model_source`·`evaluate_contract`·`resolve_topology`)와 레거시 모양을 답하는 정확한 compat owner. 이를 가능하게 하려고 `policy_registry.py` 의 어설션 id 추출기에 `.rs`(최상위 선언만) 를 더했다. 세 튜플 연결: Rust `VALID_MODES` const(텍스트에서 파싱) · 어댑터 리터럴 · compat e84 튜플이 같은 세 모드·같은 순서여야 하며 `companion_predicate` 시험과 `predicate_MODEL_ACQUISITION_TERNARY_GATE_C1` 이 비교한다(Rust const 순서를 바꾼 사본에서 시험이 실패함을 확인: `logs/b3-negative-control.txt`).

**B4 모듈 사실.** 네 함수의 `__doc__` 은 원본 문자열 그대로이고(주석으로 위임 사실을 옮김) 비공개 추가 이름은 열일곱 개다: `_CLASSES _COMPAT_OWNER _HERE _LAUNCHER _PROTOCOL _Unwire _Wire _YAML_MODULES _ask _compat _compat_module _finish _hex_text _mark _path_wire _unsupported _walk`(원본 비공개 3개 `_git_branch _load_manifest _self_test` 는 유지). 모듈 수준에 새 비공개 이름을 더하지 않는다.

**B5 compat 원본 신원 범위(문서).** import 어댑터는 compat owner 의 신원을 검사하지 않는다 — 신원(검토된 바이트 대조)은 CLI 의 사전 분류(`LegacySourceVariant`)가 단독으로 가진다. 어댑터가 읽는 compat owner 는 런처 옆 파일이라는 위치뿐이며, 그 파일이 바뀌면 어댑터의 레거시 경로(`_git_branch`·`branch_topology_conflict`·`_self_test`·사전 분류된 `main`)가 그 바이트로 답한다. 사전 업무 신원 검사를 더한다면: 레거시 경로 호출 직전에 한 번, 바이트 해시가 검토본과 다르면 업무 없이 `NotImplementedError`(Python 재시도·CLI 소스변종 경로 변경·승인 게이트 없음). 이 소스는 그 검사를 더하지 않았다(범위 밖 · 열림).

**K08.** 실제 원본 raw(R48K08: 995 겹 안쪽 dict 의 float KEY → 인코더 줄 364 `key = _floatstr(key)`, 406×3 + 반복 991, `RecursionError`, stdout 0)를 반영했다. Q: `key_is_float`(reader 가 보관한 값이 float, 값이 없으면 repr 이 float 철자) + `write_json` 의 dict 항목과 청크 작성기 `dict()` 가 키를 값과 같은 깊이 경계(`float_too_deep`)로 센다. T: `find_failure` 의 dict 팔이 `FloatKeyTooDeep` 을 내고 프레임 형식기가 `FLOAT_KEY_IN_DICT`(줄 364)로 끝낸다 — 픽스처 id·답 표 없음. 골든 `key8.json` + `manifests/R48K08.yaml`(저장된 raw 와 Root 픽스처 바이트만; 신규 Rust 출력 불사용). CLI 골든은 221(220+1). 리스트 부모의 float 317 은 소스 파생 그대로이고 set 안의 float 는 도달 불가이다.

**`__main__` 플러시.** 닫힌 표준 descriptor 로 `sys.stdout`/`sys.stderr` 가 `None` 이면 어댑터의 `__main__` 이 exec 전에 `AttributeError` 로 죽던 것을, 없는 스트림만 건너뛰고 실제 flush 오류는 그대로 올리도록 고쳤다(Python → Bash → ELF 전달은 무조건).

**열림.** U2 · U3 · set 멤버 순서 · 닫힌 fd 0 · compat 신원(B5) · Root 캡처(후보 87 외).

## R50 — `scan_node.py` P1(순수 판정·manifest 조립·파일 판독 16개 공개 이름): T 순수 모듈 + B 파일·시계 경계 + 비공개 브리지 + 얇은 Python 어댑터

상태: **P1 소스 구현 + 자체 검사(managed Rust 1.99.1 · 오프라인 · 자기 cache/target).** **전체 스캐너가 아니다 · Python-free 가 아니다.** 하드웨어 프로브·SSH/네트워크 수집·argparse `main`·`_self_test`(P2/P3/CLI)는 이 단계에서 포팅하지 않았고 명시적 compat 경로로 정확한 원본 소유자(`scan_node_original.py`, 바이트 동일, sha256 `2d9e8a86…`)가 처리한다. 이 절은 통합·독립 검증·정본 채택·Main/Sub 전달·성능을 주장하지 않는다. Root 의 어댑터 107 신규 캡처는 이 절의 범위 밖이다.

**범위.** T: `.claude/rust/role-contract/src/scan_node.rs`(순수 판정 11개 + `Raised`/`Failure` 타입과 Python 동적 연산 보조), `scan_node_manifest.rs`(`build_sub_manifest`·`emit_manifest_block` + `Clock` 경계), `scan_node_files.rs`(`cross_validate`·`read_manifest_field`·`read_manifest_topology` + `Files` 경계·UTF-8 strict·universal newline) + `lib.rs` 세 줄. B: 새 `[[bin]] scan_node`(일반 bin +1 → **cargo metadata 로 도출한 개수 55**), `src/scan_node_boundary.rs`(`StdFiles`·`LocalClock`·`FixedClock`), `src/scan_node_bridge.rs`(비공개 브리지 `scan-node-bridge/1` + 경로 관측), `src/bin/scan_node.rs`, 검사 2(`checks/scan_node_p1_original_cases.rs`·`checks/scan_node_boundary_cases.rs`)와 동결 자료 `checks/data/scan_node/p1_golden.json`. 런처 `terraforming_node/scripts/scan_node.sh`, 어댑터 `scan_node.py`, compat 소유자 `scan_node_original.py`. 새 의존성 없음(T 는 Q 의 `pydyn`·`pyops`·`pysort`·`pynumber`·`pystr` 를 재사용; B 의 `extern "C"` 는 glibc `time`·`tzset`·`localtime_r` 뿐).

**원본 → Rust.** 공개 이름 16개와 도달 가능한 헬퍼(`_parse_nccl_lines`·`_read_manifest_line`·정규식 4종의 술어화) → `role_contract::scan_node{,_manifest,_files}`. 값 의미는 `PyValue` 로 보존한다: `a or b` 가 피연산자를 돌려줌, `is False` 동일성, `in` 닫힌 튜플의 동등, `bool` 이 `int`, dict 삽입 순서, f-string `str()`, `isinstance`, 그리고 원본이 가드하지 않은 모양의 `AttributeError`/`KeyError`/`TypeError`. 두 경계는 인자다: 파일은 `Files`(실제 OS 의 errno·메시지가 `ManifestUnreadable` 의 `OSError` 원인으로), 시계는 `Clock`(`%Y%m%d%H`, `localtime_r` 가 `datetime.now()` 와 같은 원천 · `scanned_at` 이 거짓이거나 `emit_manifest_block` 일 때만 읽음). 순수 함수는 어느 쪽도 보지 않는다.

**시계 계약.** 원본의 역사적 절대 시각은 새 Native 오라클이 아니다. 형식(`%Y%m%d%H` 10자리)·필드·입도·실제 로컬 시간대·각 호출의 기록된 before/after 브래킷 안의 값이 계약이다. 자체 검사는 시계 행마다 **그 행의 저장된 브래킷의 로컬 시각**을 리터럴(`FixedClock{hour, provenance}`)로 주입하고 현재 시각인 척하지 않으며, `LocalClock` 은 `date` 와 네 시간대(Asia/Seoul·UTC·America/New_York·Pacific/Kiritimati)로 따로 대조한다. 원본 시계 monkeypatch 없음·정규화 없음.

**compat 경로.** 어댑터가 정의하지 않는 이름·CLI·`main` 은 업무 전에 PEP 562 `__getattr__`/`main()` 으로 `scan_node_original.py` 를 불러 그대로 실행하며 `SCAN_NODE_ROUTE_OBSERVER` 에 `CompatOriginalAttribute`/`CompatOriginalCli` 가 `start`·`end`(SystemExit 포함)로 남는다. 네이티브 실패 뒤 Python 재시도는 없다(실행파일 부재 = 런처 상태 2). 원본 소유자 안의 P1 함수는 원본 Python 그대로 돈다 — compat 의 전체 CLI·`--self-test` 는 P1 네이티브의 증거가 아니다. compat 의 `ManifestUnreadable` 은 원본 모듈의 클래스다(어댑터의 것과 다른 객체).

**브리지 `scan-node-bridge/1`.** `SCAN_NODE_BRIDGE` 환경이 정해진 같은 실행파일(공개 인자 ✗; 환경 없는 시작은 상태 3). 요청 `{protocol, op, params}`(어댑터가 서명으로 묶은 모든 매개변수 · 기본값 포함), 응답 `returned`/`raised`(`cls`·`ctor`·`cause`)/`unsupported`. 값은 타입 태그 JSON(정수=부호+16진, NaN·inf 는 repr).

**원본 107 → Rust.** `B50/p1-107-map.json`(행별: 원본 케이스 → 공개 이름 → 브리지 op → Rust 함수 → plane → 원본 결과 → 동결 자료 → 자체 검사). 동결 자료는 승인된 Original107 의 **저장 raw** 에서 형식 변환만으로 도출했다(원본 호출·재캡처 없음; 도출기 `B50/tools/derive_p1_golden.py`, 저장소 밖). 고정 경로는 `$R50_FIXTURES` 토큰으로, 파일 행의 바이트는 컨트롤 제안의 파일 자료로 소유 fixture 에 깔고 mode 000·디렉터리 행도 재현한다. 107 = 순수 88 + 시계 4 + 파일 15; 반환 105 + 예외 2(둘 다 `ManifestUnreadable` + `OSError` 원인).

**자체 검사(실제 실행).** `scan_node_p1_original_cases` 3(107 행 전수 · 계획 개수 단언 · 하네스가 기대 변경을 잡는 음성 대조), `scan_node_boundary_cases` 6(로컬 시계 대 `date` 4개 시간대 · 파일 결함 errno/메시지 · 실행파일은 브리지 전용 · 한 요청의 응답과 `start`/`end` 관측 · 프로토콜 오류는 제어 실패), 어댑터 107 행 왕복(Python 어댑터 + 실제 실행파일; 시계 행은 실제 시계로 `scanned_at` 만 가림)과 compat 경로·부재 실행파일 확인(저장소 밖 도구), P1 문자열 리터럴 490 개의 Rust 소스 포함 확인(6 개는 Rust 이스케이프·`%d`→`{index}` 표기 차이뿐), `antipattern_scan --tripwire`/`--self-test` 통과. fmt·check·release build·clippy `-D warnings` 는 T·B 양쪽 통과.

**잔여(숨기지 않음).** (1) **반환값과 입력 객체의 동일성(aliasing)·NaN 동일성**은 재현하지 않는다 — 동결 자료의 `identity` 가 8 행을 표시한다(값은 같고 객체는 새것). (2) U2: in-process 예외의 traceback 바이트와 `AttributeError.name/obj` 는 재현하지 않는다. (3) 어댑터는 정확한 `str`·`int`·`bool`·`float`·`None`·`bytes`·`list`·`tuple`·문자열 키 `dict` 만 전달하고 그 밖(`os.PathLike`·int 경로=fd·자기 참조)은 업무 전에 `NotImplementedError`. (4) 107 행이 닿지 않는 분기(IPv6 scope `valid_host` · 유니코드 숫자의 `isdigit`/`\d` · `dict()` 의 비정상 모양 · 8192 바이트를 넘는 파일의 UTF-8 오류 · 열린 뒤 읽기 중 OS 오류 · 거대 정수의 `str()` 한도)는 **열림**이다: 원본 소스에서 옮겼고 문자열 리터럴 포함은 확인했으나 저장 raw 의 근거가 없다(일부는 `Unsupported` 로 업무 전에 거절). (5) `claim_predicates`·`companion_predicate`·`claim_bindings.json`·`registry.yaml`·`verify_distribution --self-test` 가 읽는 `scan_node.py` **소스 텍스트** 바인딩(`host_safety.installed`·`Flag 와 독립`·`BatchMode=yes`·`hf_token_env_file` 싱크 AST·emit 가드 문자열)은 어댑터 교체로 어긋난다 — P1 에 있던 문구는 Rust 로, P2/P3/CLI 에 있던 문구는 `scan_node_original.py` 로 재바인딩해야 한다(통합 단계 항목). (6) 전체 B 스위트 `cargo test --no-fail-fast` 는 573 통과 · 116 실패이며 모두 이 변경과 무관한 옛 소유자 검사의 환경 의존(NOT_RUN 51 · crate-local `target/release` ELF 요구 36+ · PTY 8 · `/dev/full` 7 등)이다. 순수 기준선과의 대조는 하지 않았다.

### R50 P1 계약 보정 — 공유 동일성 plan · `AttributeError.name/obj` · 공개 `ManifestUnreadable` 하나 · export 배관 · 네이티브 자기 검사 · 정책 문구 재결속

위 R50 절의 잔여 (1)(이름: 반환값/입력 동일성·NaN 동일성)·(2)의 `AttributeError.name/obj`·`ManifestUnreadable` 공개/compat 분리·`from scan_node import *` 차이는 **측정된 107 계약 안의 결함**이라 보정했다(선언된 예외로 두지 않는다). U2 traceback 프레임은 열림이다 — 위조하지 않는다.

**8행 분류(저장 raw 의 그래프 ref).** 6행 — `S50N01·N02·N03·N05·N06·N07`(`cross_validate`): 반환의 `checks[0]["scanned"]` 가 호출자 인자 `ic["hca_devices"]` 와 **같은 list 객체**(raw `ref` → 인자 그래프, 경로 `ic → hca_devices`). 2행 — `S50B08`(`emission_blockers`)·`S50E17`(`evaluate_gate`): 인자에 NaN 객체(raw `nan_id` 1)가 있으나 반환에는 그 NaN 도 어떤 호출자 객체도 없다(반환은 새 list/dict 들; 반환→인자 ref 0). 반환 내부 공유 ref 는 107 행 전체에서 0. 동결 자료의 행별 `identity`(`returned_argrefs` 경로·`arg_nan_identity`)와 기대 반환의 `argref` 경로는 저장 raw 에서 도출한다(도출기 `B50/tools/derive_p1_golden.py`; v1 은 `derive_p1_golden.v1.py`).

**공유 동일성 plan(`scan-node-bridge/1`).** 어댑터는 호출자 인자의 **번호 붙은 객체**(`None`/`bool` 을 뺀 모든 객체: 컨테이너·NaN·문자열·정수·실수·바이트)를 전위 순회 순서로 `"id"` 와 함께 보내고 반복 방문은 `{"t":"ref","id":n}` 로 보낸다. Rust 는 그것을 공유 값(`PyValue::Shared`)으로 해독하므로 업무 함수가 인자를 그대로 넘기면(`cross_validate` 의 `scanned`, `evaluate_gate` 의 `per_port_gbps`, `build_sub_manifest` 의 `custom_model_paths`·`scanned_at` 등) 같은 객체가 돌아가고, 응답은 그것을 `{"t":"argref","id":n}` 로 적는다. 어댑터는 `argref` 를 **호출자의 실제 객체**로 조립할 뿐이다(복사 ✗ · 선택 ✗). 새로 만든 값은 새로 만든다. 순환·비문자열 dict 키는 업무 전에 `NotImplementedError`(키 문자열의 객체 동일성은 전달하지 않는다 — 잔여).

**`AttributeError`.** T 의 `Raised` 가 `ctor_kwargs`(`name`·`obj`)를 갖고(`Raised::attribute`), 실패한 읽기의 대상 객체를 값(공유 값 그대로)으로 싣는다 — 호출자 객체이면 `argref` 로 되돌아가 `exc.obj is <호출자 객체>`. 어댑터는 `AttributeError(msg, name=…, obj=…)` 로 재구성한다. 저장 raw 에는 이 예외가 없다 → 기대는 같은 객체에 같은 읽기를 CPython 이 일으키는 `AttributeError`(args·name·obj 동일성)이며 Rust 출력을 오라클로 쓰지 않는다.

**공개 `ManifestUnreadable` 하나·export 배관.** 어댑터는 모듈을 올릴 때 정확한 원본 소유자를 모듈 이름 `scan_node` 로 실행해(스택 프레임 파일은 원본 그대로) 그 이름을 이 모듈에 배관한다(어댑터가 정의한 P1 이름은 그대로 네이티브). 그 결과 `from scan_node import *` 의 export 집합(실측 56 = 56)·함수 서명·`__module__`·`__qualname__`·정규식 상수·`__doc__` 가 원본과 같다. 공개 `ManifestUnreadable` 은 어댑터의 클래스 하나이며 docstring 은 원본의 것이고 compat 전역(`ManifestUnreadable`)에도 같은 클래스를 꽂는다(원본 소스 불변). 어댑터 고유 이름은 `_sn_`/`_SN_`/`_Sn` 접두 19개뿐이다. 이름공간의 `__file__`/`__spec__` 같은 던더는 어댑터의 것이다(원본 107 import probe 의 `module_file` 이 소유자 경로이기 때문).

**시계 검사(별도 보존).** 이전의 마스킹 검사(`adapter_roundtrip.masked-v1.py`·로그)는 더 좁은 증거로 **그대로 보존**한다. 현행 검사는 원문 출력을 보존하고(`logs/adapter_clock_qualified.raw.json`), 원본 소스가 정의한 형식(`%Y%m%d%H` 10자리)·시간 입도·프로세스 로컬 시간대(Asia/Seoul·UTC·America/New_York·Pacific/Kiritimati)와 **각 네이티브 호출의 실제 before/after 브래킷**으로 판정한다. 과거의 절대 시각을 오라클로 쓰지 않고 관측 시각을 채우지 않으며 원본 시계를 패치하지 않는다. Rust 쪽 단위 검사의 시계는 B→T 경계에서 주입한 리터럴(`FixedClock`)이다.

**네이티브 자기 검사.** `scan_node --p1-self-check`(`scan_node.sh --p1-self-check`)는 내장된 저장 raw 107 행(순수 88·시계 4·파일 15)을 브리지로 돌려 값·예외·`argref` plan 을 대조한다. 레거시 `scan_node_original.py --self-test` 의 증거가 아니며 P2/P3/CLI 를 보지 않는다. `build_rust.sh` 가 빌드 뒤 실행하고 `verify_distribution` 은 레거시(`terraform_scan_selftest_legacy`, 원본 소유자 직접)와 네이티브(`terraform_scan_p1_native_selfcheck`)를 따로 둔다.

**정책 문구 재결속.** 문구가 P1 에 있던 것(`host_safety.installed`·`Flag 와 독립` = `emit_manifest_block`)은 T Rust `scan_node_manifest.rs` 를, 미포팅(P2/P3/CLI)에 있던 것(emit 가드 문자열·`BatchMode=yes`·`host-key 미등록/불일치`·`키 인증 거부`·`hf_token_env_file` 싱크 AST·`collect_peer_hw`·`_peer_ssh_diag`)은 정확한 `scan_node_original.py` 를 읽는다. 토큰 파일 내용 비열람 불변식은 원본 텍스트(`open(hf_token_env_file`·`read_text` 부재, 원격 존재검사)에 더해 P1 Rust T 소스(`std::fs`/`File::open`/`read_to_string` 부재, `hf_token_env_file` 은 따옴표 친 manifest 키로만, 파일 판독부는 토큰 포인터를 아예 모름)로도 판정한다. `claim_bindings.json`·`registry.yaml`·`base_skill_edges.json`·`companion_predicate.py` 를 같이 맞췄고 어떤 tripwire 도 약화·삭제하지 않았다.

**잔여(숨기지 않음).** (1) U2 traceback 프레임(원본 소스 줄)은 재현하지 않는다 — 열림. (2) 키 문자열의 객체 동일성. (3) 어댑터가 전달하지 않는 값(`PathLike`·int 경로=fd·순환·비문자열 키)은 `NotImplementedError`. (4) 107 행이 닿지 않는 분기는 열림(위 R50 절 (4)). (5) compat 쪽 원본 함수(미포팅 P2/P3/CLI)는 원본 Python 그대로이고 그 안의 P1 호출도 원본 Python 이다. (6) 정책 레지스트리 `verify` 는 새 파일이 **미추적**이라 `EVIDENCE_UNTRACKED` 2건(`scan_node_manifest.rs`·`scan_node_original.py`)을 내며 추적(커밋)되면 해소된다. P2/P3/CLI 전체는 미포팅이다.

## R48 — API v5 실측 차이 3종의 소스 교정 제안 (LM10~13 · RT09 · MF01) — 제안, 통합 전

실제 후보 첫 캡처(`r48-api87-held-actual-*`: 87행 중 81 전 필드 동일)의 불일치 6행에 대한 소스 제안이다. 닫힌 Original 은 재생하지 않았고 불일치 면제·수용 주장은 없다.

- **LM10~13 인자 길이.** 실제 Original 의 YAML 표식 예외는 `args` 길이 4(어댑터는 `note=None` 을 덧붙여 5). 설치된 PyYAML 6.0.3 의 composer/constructor/parser/scanner `raise` 지점은 전부 위치 인자 정확히 4개이며 `note` 를 넘기는 곳이 없다(AST 전수: composer 3 · constructor 30 · parser 10 · scanner 31 = 74개). 브리지 답이 `args_arity`(4, note 가 실제로 있으면 5)를 실어 어댑터가 `cls(*parts[:arity])` 로 원본과 같은 길이로 만든다. 속성(`context`/`problem`/`note`/마크)은 종전 그대로. **repr 의 `<yaml.error.Mark object at 0x…>` 주소는 프로세스마다 다른 본질적 비결정이다 — 열림**(주소를 꾸미거나 정규화하거나 필드를 빼지 않았다; 자체 검사는 두 신규 프로세스의 어댑터 주소가 서로 다름을 기록한다). `args` 의미 교정과 repr 비결정은 별개 항목이다.
- **RT09 호출자 PATH.** 브리지 시작은 호출자의 업무 PATH 에 의존하지 않는다: 어댑터는 절대 경로 `/bin/bash` 로 런처를 시작하고(이미 `__main__` 이 쓰던 경계), 런처는 외부 프로그램 없이(`dirname` 대신 `${BASH_SOURCE[0]%/*}` + 빌트인 `cd`/`pwd`) 자립한다. 호출자 PATH 는 바꾸지 않으며 `git` 자식은 그대로 받는다 — 그래서 PATH 에 git 이 없으면 Rust 프로브가 원본처럼 `None` 을 돌려준다. B1(배관 복원·unset 대 빈 값) · 0600 임시 요청 파일 · stdin FD · 지연 import 는 불변.
- **MF01 모듈 이름공간.** 열일곱 개 구현 도우미(`_Wire`·`_Unwire`·`_ask`·`_finish`·… 이전 B4 문단의 목록)는 `_build()` 의 **클로저 안**으로 옮겼다. 아홉 개 공개 함수는 그 빌더가 만든 보통의 모듈 수준 함수 객체이고 이름 대입 뒤 빌더는 `del` 된다. 모듈 이름공간은 원본 25개와 같다. 기계적 메타데이터는 하나뿐이다: 빌더 안에서 만들어진 함수의 `__qualname__` 을 평범한 이름으로 설정(빌더가 지워진 뒤의 수출 이름). `__getattr__`/`__dir__`/globals 조작/관찰 가로채기 없음. 정적 소비자가 `FunctionDef` 로 어댑터의 함수를 찾는다면 이제 이름 대입으로 보인다(발견된 소비자 없음). **이 문단이 이전 B4 의 "비공개 추가 이름 17개가 모듈 수준" 서술을 대체한다.**
- **열림.** U2(in-process traceback 프레임) · U3(지원 타입 밖 인자) · repr 주소 비결정 · set 멤버 순서 · 예약 `MANIFEST_CONTRACT_CALLER_*` 이름 · 닫힌 caller fd0(`/dev/null` 로 재개방) · Root 캡처는 이 제안의 범위 밖.

## R48 v8 — 공개 `manifest_contract.py` CLI 는 Python → Rust 직접 exec, D1/D2 정책 정합

- **공개 진입.** v7 까지 `manifest_contract.py` 의 `__main__` 은 항상 `/bin/bash manifest_contract.sh` 를 거쳤다. 호출자가 인터프리터 시작 뒤 cwd 를 지우면(원본은 파이썬 직접 시작) 셸이 `shell-init`/`chdir` getcwd 경고 두 줄을 stderr 에 먼저 쓴다. v8 은 실행파일이 있을 때 같은 PID 로 Rust 에 직접 exec 하고(`manifest_contract.sh` 가 만드는 환경과 같다: 운반 변수·기본값·unset 규칙), 부재·실행 불가·`VALIDATE_ONLY=1`·exec 실패는 런처로 넘긴다(런처 불변). 파이썬은 업무 판정·대체·프레임 위조를 하지 않는다. 설계·검증 원본: B50 `v8-mc-public-entry-and-cli-control-proposal/public-entry`(c264); 어댑터의 공개 이름 25개·함수 9개·B1~B4 불변.
- **D1** `base_skill_edges.json`: v4 B3 가 정책 증거를 `manifest_contract_compat_owner.py` 로 옮긴 뒤 원장이 갱신되지 않았다 — 3행 재지정 + `claim_predicates.py → compat owner` 1행 추가(다른 간선 불변).
- **D2** `checks/data/scan_node/p1_golden.json`: `provenance.owner_sha256`(추적 파일 `scan_node_original.py` 의 digest 재기재)만 삭제 — 107행 골든·원시 맹점층 참조(inputs/summary digest)는 보존. 이 파일은 `include_str!` 로 박히므로 B scan_node 산출물은 새 소스에서 다시 빌드해야 한다(옛 ELF 를 새 소스의 증거로 쓰지 않는다).

## R50 P23 review fixes (Builder 73903c97, private proposal on frozen v8; not integrated, not R50 acceptance)

- **New modules (c264 proposal, kept):** `role-contract/src/scan_node_probe.rs` (P2 probe decisions over the `Host` trait), `src/scan_node_host.rs` (the real host: processes, files, sockets, HTTPS), `src/scan_node_cli.rs` (P3 command line over the same trait), `src/help_layouts_scan_node.rs` (source-derived help layouts, see NOTICE). `scan_node_bridge.rs`/`bin/scan_node.rs`/`lib.rs`/`scan_node.py` carry the typed P2 bridge and the P3 route.
- **Cargo.** The existing dependency `rustix` gains the feature `system` (`uname().nodename()`); no new crate, the three `Cargo.lock` files are unchanged (`--locked` passes). Three `[[test]]` registrations of the proposal plus one for the review-fix regression.
- **Review fixes.** D1: every option the original tests with `if args.opt:` (`peer_ip`, `peer_ssh`, `sub_work_dir`, `emit_sub_manifest`) is read through a Python-truthiness view at every decision site (the raw value is still echoed where the original echoes `args.peer_ip`). D2: `Local::user/cwd` return typed results: a removed cwd ends like the original's uncaught `FileNotFoundError` (no stdout), a `getpass` miss is an explicit unsupported boundary (NSS is not modelled, no `KeyError` is invented). M4: both streams are decoded before the return code of `ip`. M5: the launcher and the adapter record the caller's plumbing values (`SCAN_NODE_CALLER_*`, flag `SCAN_NODE_CALLER_ENV`) and the native host hands them back to probe children. M6: the `unsafe impl Sync` is gone (not needed). M7/M8: documentation addresses, no case id in production text. M9: the help-layout generator stays outside the repository. M11: the adapter check is embedded in its single Rust test.
- **Residuals M1/M2 (this change).** M2: `scan_node.py` decides BEFORE any Rust process exists whether an argv token selects the original's `--self-test` by argparse's own rule (the exact name, or a prefix of it that no other option shares: `--se`..`--self-tes`, with or without `=value`); a selected argv goes to the exact original owner (which then runs its self-test or reports its own parser error), ambiguous (`--s`) and invalid prefixes stay on the native CLI, which ends them in the stdlib parser errors. The option list is checked against the original source by its own test; no handoff after Rust started, no fallback. M1: the original writes every diagnostic (stderr) before its first output (stdout); the port collects both, so it now writes the stderr text first. The earlier statement that merged pipes were unaffected was wrong for an output that overruns the stdout block buffer (the first block preceded the diagnostics): the own test reproduces that on the previous code. A real terminal run could not be made (no pty device in the guard: NOT_RUN); the order is proven on the merged stream, the live timing of the diagnostics on a terminal is not reproduced.
- **Still open.** U2 (traceback frames), the NSS-only `getpass` boundary (explicit, not implemented), and host-observed rows are skipped off aarch64 and as root (a green run elsewhere proves less).

## R51 — 이 스킬의 실행·검사 경로에서 Python 제거 (plan_26111615 P1 · 2026-11-16)

이 스킬의 런처·Rust 실행파일·자체검사·`cargo test` 가 **이 스킬의 `.py` 를 실행하지 않게** 했다. `.py` 는 지우지 않았다(삭제는 라이브 증거 뒤 P6).

- **반복 축 캠페인 소유자(`repeat_owner`).** Python 질의 어댑터 경로(`LegacyExplicit`·`LegacySourceVariant`)와
  `REPEAT_AXIS_CAMPAIGN_ADAPTER`/`REPEAT_AXIS_PYTHON`/`REPEAT_AXIS_COMPAT_ADAPTER` 를 걷어냈다. 해소자가 설치돼 있으면
  (`campaign_init.py` **또는** 런처 `campaign_init.sh` 가 정규 파일) 소스 바이트와 무관하게 role-contract 읽기면이 답한다.
  둘 다 없으면 종전대로 부재. 런처 4종·`build_rust.sh` 의 어댑터 export 삭제. 경로 관측 줄의 필드는 유지(`python_child` 는 항상 false).
  원본 대조 검사 `native_and_the_live_adapter…` 는 커밋 골든 `checks/data/repeat_axis/r44_native_goldens.jsonl` 대조로 바꿨다 —
  골든은 직전 커밋(b1f3563e)에서 같은 검사가 live 어댑터와 답 하나하나 같다고 단언한 native 세션의 답이다. 소스 변종 골든
  `owner_variant_goldens.jsonl`(Python 원본이 변종 소스를 import 한 결과 60행)은 그 경로가 없어져 삭제했다.
- **인증서 발행의 캠페인 writer 호출.** `python3 campaign_init.py --evidence-add` → `bash campaign_init.sh --evidence-add`
  (`PUBLISH_BENCHMARK_PYTHON` 삭제). 부재 판정·안내 경로도 런처 기준이다. `reporting/cli_cases.json` 의 캠페인 owner 사본
  (`campaign_init_original.py` 참조 6곳)은 rc 0 런처 stub 으로, 부재 안내 4곳은 런처 경로로 갱신 — `campaign_init_original.py` 는 더 쓰이지 않는다.
- **자체검사 모래상자의 stub.** run_bench/lite_bench shim 의 Python heredoc → 새 실행파일 `selftest_load_shim`(`run-bench`·`lite-plan`) ·
  judge_bench 발행기 stub·Flag 게이트 stub → bash. PyYAML 탐침(`interpreter_has_yaml`) 삭제(셀 writer 는 native YAML).
  사본 목록에서 이 스킬의 `.py` 전부 제외. 고정 사례의 가짜 `docker`(Python) → `selftest_fixture_docker`(`dataset`·
  `dataset-stdout`·`dataset-stdout-stderr`·`bench-tool`)를 exec 하는 2줄 셸 · 경로는 `SELFTEST_FIXTURE_DOCKER_BIN`(검사 공통 `populate` 가 설정).
- **full ⊇ lite 검사.** 존재 행 `lite_metrics_native.py` → `lite_metrics_api.sh`, `E② native 분류기 Python 호출 경로` → 런처
  `classify_cell.sh` (고정 출력 `normal_stdout.txt` 2행 갱신). campaign writer 의 lite 판정 소유자 존재 판정도 `lite_metrics.sh`.

**남은 Python(다른 소유자 쪽 · 이 스킬 단독으로 못 닫는다).** `owner_bridge`(hint reader·evidence publisher·completion gate 를
Python 세션으로 부른다 — 그 소유자들이 Rust 가 되면 사라진다 · `selftest_lite_report` 는 evidence publisher → `policies/runtime/doc_naming.py`
→ 이 스킬의 `doc_naming_native.py` import 사슬 때문에 이 스킬 `.py` 없이 실패) · `resolve_topology` 의 원본 변종 호환
(`--compat-python` · 명시 인터프리터 · 고정 사례의 stand-in 판정자가 Python 프로그램 — topology_parity 소유자 계약과 함께 바뀌어야 한다;
`topology_legacy_api.py` 도 같은 계약) · `run_bench.sh` 의 `blackbox_session.py` 직접 호출 · `campaign_init.sh` 의 미모델링 동사
(`--evidence-add`) · 다른 스킬의 Python 호출자가 import 하는 `*_native.py`(hintlib·evidence_publisher·doc_naming·escalation_predicate,
그리고 explorer Rust 의 `classify_cell_native.py` 존재 판정).
