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

## roofline · verdict_rule (R3 기능 통합 PASS · 연산 성능 개선 중)

범위는 `scripts/roofline.py`(476줄)와 `scripts/verdict_rule.py`(981줄) 전체 — 함수/API와 CLI — 및 `judge_bench.sh`의 `SPEC_ROW`
계산과 `broad_search.sh`의 `rubric_authority` 확인이다. 원본 `.py`는 대조용으로 유지한다. 다른 Python 도구·실제 서빙 측정의 이행 완료가 아니다. 후보 자체검사·독립 표적 검증 이후 정본에 연결했고 아래 정본 통합 검사도 통과했다. 전체 연산 성능 목표는 아직 충족하지 못했다. 초기 I1/I2/Y1 및 후속 I3/I4/I5 실패 원시는 보존하고 같은 입력의 재검증으로 해소했다.

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
