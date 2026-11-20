# 네이티브 타입 계약 (runtime-support · 16 크레이트 공통)

- 정본: plan 26111805 「보존 계약」·「cell 세부」 c20, interview D1·D3·D4·D7·D8·D9·D16·D17, 사용자 결정 h9–h16.
- 쓴 cell: c20-contract (base `ead7087d`, 3회차는 g-c0 검토 1회차 지적 반영). 읽는 cell: c21(계약 시험), c22(yaml_compat), c30~c60(이전·삭제).
- 같은 폴더: `consumers.tsv`·`consumers.md`(고정 표면과 소비자), `entrypoints.tsv`(결정론 진입점),
  `yaml_inputs.tsv`(YAML 1.1 의존 입력), `divergence_kinds.md`(divergence 의미 분류).
- 크레이트 판·라이선스는 `.claude/rust/py-boundary-guard/crate_allowlist.tsv`가 정본이다. 이 문서는 어떤 크레이트를 어디에 쓰는지만 적는다.
- 이 문서의 결정은 c20 검토(judgment)로 수용한다. 크레이트 판·라이선스는 h9로 승인됐다(`crate_allowlist.tsv` 85행과 허용 라이선스 집합).
  바꾸려면 c20을 다시 연다(c21도 다시 연다).
- 런타임 환경 모드(container|native)의 닫힌 값·판정·린트와 upstream의 native 빌드 헌장은 이 문서가 아니라 `c35-runtime-mode`가 맡는다(h16 스레드 사용자 요청).

## 1. 보존 기준과 출력 호환 수준

rust-repack의 기본값 "외부 동작 보존"을 **고정 표면**(D8)으로 한정한다. CPython 의미(문자열 = 코드 포인트, `repr`, `json.dumps` 바이트,
PyYAML 1.1 해소, argparse 문구, Traceback 흉내)는 보존 대상이 아니다(D1·D3).

| 수준 | 대상 | 이전 뒤 요구 |
|---|---|---|
| **L1 바이트 고정** | 종료 코드 값, 표식 줄(`…-PASS` 등), reason_codes 문자열, 기계 파싱 표(SKILL.md §4 공개 표면 표 등), tripwire가 읽는 stderr 줄, 셸 소비자가 grep·awk·sed로 읽는 줄, 서브커맨드·플래그 long 이름과 실제 쓰는 짧은 형, 런처 환경 변수 이름과 그 값의 해석, argv[0]으로 루트를 찾는 bin의 argv[0], 다른 크레이트가 소스 텍스트로 대조하는 토큰 | 같은 바이트. 바꾸면 같은 cell에서 소비자를 고치고 divergence `fixed-surface`(의미 `surface-change`) 행 |
| **L2 값 고정** | JSON 파서가 읽는 stdout·파일(키, JSON 타입, 값), stdout/stderr 분리 | 파싱한 값이 같다. 키 순서·공백·이스케이프·float 표기는 자유. 바이트 비교 골든은 재기준선(divergence `golden`·`parity`) |
| **L3 자유** | usage·help·오류 문구와 배치, 접두어 축약, Python repr·Traceback 흉내, 진단 메시지 | 소비자가 없으면 자유. 바뀐 골든은 `golden` 재기준선 |

- 어느 출력이 L1·L2인지는 `consumers.tsv`의 `reads` 열이 정한다.
  - `stdout-text`·`stderr`·`merged-output`·`source-text`, 텍스트로 읽는 `file:`은 L1이다. 셸 소비자가 JSON 출력을 텍스트로 grep·sed하면 그 줄은 L1이다.
  - `stdout-json`과 JSON 파서로 읽는 `file:`은 L2다. `exit:`는 L1이다. `golden(…)`·`comment`·`agent-instruction`은 L3이다(단 `fixed`가 `argv`이면 서브커맨드·플래그는 L1).
  - 확인되지 않은 행은 L1로 다룬다. `reads`가 `?`(252행)이거나 `…?`(108행, 대부분 `stdout-json?`)인 행, `fixed`가 `?`(25행)인 행이다.
    이전 cell이 소비자 코드를 읽고 낮출 수 있다. 낮추면 그 행을 `checked`로 고친다.
- 접두어 축약(argparse의 `--rep` → `--repo`)은 자유다. 저장소 안 소비자가 축약형을 쓰면(`consumers.tsv` `invocation`) 그 형을 명시 별칭으로 남긴다.
- 반복 플래그와 음수 값(clap으로 옮길 때):
  - argparse `store`는 같은 옵션을 두 번 주면 뒤 값이 이기고, `-1`처럼 `-`로 시작하는 음수를 옵션 값으로 받는다.
  - clap derive의 기본은 단일 값 옵션의 반복을 오류로 보고, `-1`을 모르는 플래그로 읽는다(`allow_negative_numbers`·`allow_hyphen_values`가 필요하다).
  - 소비자가 반복이나 음수 값을 넘기는 bin(`consumers.tsv` `invocation`)은 지금처럼 받게 설정하고 계약 시험에 그 사례를 둔다.
  - 소비자가 쓰지 않는 반복을 오류로 바꾸는 것은 divergence `output`(의미 `strictness`)이다.
- 환경 변수와 argv[0]: 런처 환경 변수(`<NAME>_BIN`, `TERRAFORMING_BIN_DIR`, `EASY_VLLM_UPSTREAM_RUNTIME_ROOT` 등)는 이름과 값의 해석이 L1이다. 해석에는 설정 안 됨·빈 값·상대 경로의 구분이 든다.
  예: `inline_json.sh:8-15`는 `EASY_VLLM_UPSTREAM_RUNTIME_ROOT`가 설정됐는데 비면 오류다. argv[0]으로 루트를 찾는 bin은 런처가 `exec -a`로 넣는 값을 지금처럼 해석한다(`consumers.md` 「런처와 환경 변수」).
- L2 값과 CPython 의미의 경계:
  - JSON 값 자체가 CPython 의미로 만든 문자열일 수 있다. 예는 `repr(float)`·`str()` 결과를 담은 문자열 값과 Python 정렬 순서로 만든 목록이다.
  - 그 값이 바뀌면 L2 값 변경이다. 소비자가 그 값을 비교·파싱하면 `surface-change`이고, 사람만 읽으면 `cpython-semantics`다. 근거는 `consumers.tsv` 행이다.

## 2. 텍스트와 OS 문자열

| 값 | 타입 | 비UTF-8을 만나면 |
|---|---|---|
| 기계 텍스트(JSON·YAML 값, reason code, 메시지, manifest 값) | `String`·`&str` | 경계에서 오류(입력 오류 → 종료 2). surrogateescape로 숨겨 나르지 않는다 |
| 식별자(node_id, topology, campaign·cell id, 모델 이름, 태그 이름, 브랜치 이름) | `String`, 경계에서 닫힌 문자 집합 검사(지금 각 bin의 정규식) | 입력 오류(종료 2). 경로 조각으로 쓸 때만 `Path::new(&id)` |
| 파일 경로·파일명 | `PathBuf`·`&Path`(`OsString` 기반) | 그대로 나른다. `String`으로 바꾸지 않는다 |
| 경로를 기계 출력(JSON 값·키, L1 줄)에 쓸 때 | `path.to_str()` | `None`이면 오류(입력 오류 → 종료 2). `to_string_lossy`로 U+FFFD를 섞어 내지 않는다 |
| 경로를 사람용 메시지에 쓸 때 | `path.display()` | 손실 표시 허용(L3) |
| argv | `std::env::args_os()` → clap(`value_parser!(PathBuf)`는 경로, `String`은 텍스트) | 텍스트 인자의 비UTF-8은 clap 사용법 오류(종료 2). `std::env::args()`는 쓰지 않는다(비UTF-8에서 panic) |
| 환경 변수(경로) | `std::env::var_os` | 그대로 `PathBuf` |
| 환경 변수(텍스트) | `std::env::var` | `Err(NotUnicode)`는 오류로 보고한다. 없음(`NotPresent`)과 섞지 않는다 |
| 바이트 데이터(해시 대상, 로그 원문) | `Vec<u8>`·`&[u8]` | 해당 없음 |
| 바이트 스트림 경로(stdin→stdout 원문 전달, 파일 목록을 줄로 나르는 파이프, 해시·복사 대상) | `std::io::Read`/`Write`·`Vec<u8>`, 줄 나눔은 `split(b'\n')` | 텍스트로 디코딩하지 않는다. 원문 바이트를 그대로 나른다 |
| 로그·진단에 외부 텍스트를 담을 때(자식 프로세스 stderr 꼬리 등) | `String::from_utf8_lossy` | 허용(L3). 단 그 줄을 소비자가 기계로 읽으면(L1) 손실 변환을 쓰지 않는다 |

- 지금은 세 관례가 섞여 있다: surrogateescape `PyStr`(sweep-stop 41·recipe 17·upstream 10 파일), `to_string_lossy`(policy-gate·policy-selftest·terraforming),
  `std::env::args()`(policy-audit·distribution·publisher·predicates·hint-publisher·parity와 `*_api`·`*_bench` bin들). 이전한 크레이트는 위 표 하나로 맞춘다.
  py-boundary-guard `crate_allow`는 3회차에 `args_os`로 바꿨다(비UTF-8 인자·추적 lock 경로 → 종료 2).
- `PyStr`이 담던 홀 서로게이트(U+D800..DFFF)는 `String`에 없다. 저장소의 입력(manifest·원장·캠페인 상태·argv)에서 홀 서로게이트를 일부러 나르는 계약은 찾지 못했다.
- c21 a·b·c가 실제로 고정한 비UTF-8 사례:
  - 텍스트 인자(종료 2, stdout 빈):
    - c21 a: policy-gate의 `completion_gate`·`doc_naming`(`policy-gate/checks/contract_checks.rs:310`).
    - c21c: policy-selftest의 `runtime_selftest`·`antipattern_scan`·`constitution_lint`(`policy-selftest/checks/contract_checks.rs:170`).
    - c21c: adversarial-benchmark의 `verdict_rule`(`contract_checks.rs:1549`)·`manifest_contract`(`contract_checks_campaign.rs:404`)·`topology_parity`(`:682`)·`campaign_init`(`:964`).
    - c21b(recipe·upstream·terraforming)는 0이다.
  - 파일명: 하나뿐이다. adversarial-benchmark `resolve_topology --repo <비UTF-8 디렉터리>`가 경로를 나르고 `multi`를 낸다(`contract_checks.rs:1421`).
  - c20: `crate_allow`의 비UTF-8 인자·추적 lock 경로(`py-boundary-guard/checks/crate_allow_checks.rs`).
  - 나머지 bin에는 사례가 없다.
- **이전 cell 의무:** `std::env::args()`·lossy 경로를 `args_os`·`Path`로 바꾸는 cell은 그 bin의 계약 시험에 다음을 더한다.
  - 비UTF-8 텍스트 인자 사례(종료 2, stdout 빈).
  - 경로 인자가 있으면 비UTF-8 파일명 사례(경로로 나르고, 기계 출력에는 쓰지 않거나 종료 2).
  - 이 사례는 빼거나 약하게 하지 않는다. 빼면 `test_delta.tsv` 대상이고, 승인 없이는 gate 규칙 4가 막는다.
  - c30은 소비 크레이트에서 `std::env::args()`·`to_string_lossy`가 기계 출력에 닿는 자리를 찾는 정적 검사를 둘지 정한다(제안).
- 대소문자·공백 판정은 Rust 표준(`char::is_whitespace`, `to_lowercase`)을 쓴다. CPython 3.12 Unicode 15 표(`pylower`, `unicode_*_table.rs`)는 버린다.
  비교 결과가 달라지는 자리는 divergence `output`·`cpython-semantics`다.
  - 공백의 차이: Python `str.isspace()`·`str.split()`은 U+001C–U+001F(정보 구분자)를 공백으로 보지만, Rust `char::is_whitespace`와 regex 크레이트의 `\s`(Unicode White_Space)는 보지 않는다.
  - L1 구분자 U+001F(`bench_preset`·`repeat_axis`·`verdict_rule`·`sweep_stop` 출력, `consumers.md` 「고정 표면」)는 `split('\u{1f}')`로 명시해 나눈다. 인자 없는 `split()` 흉내로 나누지 않는다.

## 3. 오류와 종료 코드

- `main`은 `std::process::ExitCode`를 돌려준다. 종료 코드는 `u8` 범위의 값으로만 만든다. `SystemExit` 흉내(`& 0xff` 대 `i32::try_from`의 갈림)는 버리고,
  bin마다 실제로 쓰는 코드 집합을 계약 시험이 고정한다(`consumers.md` 「종료 코드」).
- 공통 의미(지금 관례 그대로): 0 통과·성공, 1 검사 실패·위반, 2 사용법 또는 입력 오류(파싱 불가, 읽을 수 없는 파일, 비UTF-8 텍스트). 그 밖의 코드(예: lite/sweep 6·7)는 bin별 고정 표면이다.
- 여러 실패가 한 실행에 겹칠 때 어느 코드를 내는지(우선순위)는 bin별 고정 표면이다. 지금 코드의 검사 순서를 그대로 옮긴다. 순서를 바꾸면 소비자가 구분하는 코드(`consumers.md` 「종료 코드」)가 달라지는지 보고, 달라지면 `surface-change`다.
- 자식 프로세스가 신호로 끝난 것을 보고하는 표기는 지금 bin마다 다르다(Python `returncode` 음수 흉내, `128+N`, 문구 등 — 검토 T-N1).
  - 소비자가 그 값을 기계로 읽는 자리만 L1이다. 이전 cell은 `ExitStatusExt::signal()`로 받아 그 bin의 지금 표기로 낸다.
  - 소비자가 없으면 L3이다.
- panic은 종료 101이다. 어느 bin의 계약 코드도 아니므로 소비자에게는 "정하지 못함"(비0)으로 읽힌다. 입력에서 닿는 panic(`unwrap`, 범위 밖 첨자)은 결함이며 입력 오류(종료 2)로 바꾼다.
- stdout이 닫힌 파이프(EPIPE)일 때 `println!`은 panic한다. 셸 소비자가 `| head`·`| sed … | head -1`로 읽는 bin(`consumers.tsv` `invocation`)은 쓰기 오류를 받아 조용히 끝낸다.
  끝내는 종료 코드는 그 bin의 정상 코드다. Python 판의 BrokenPipeError 동작은 보존 대상이 아니다(L3).
- 오류 값은 크레이트마다 `enum`(+ `std::error::Error`·`Display`)으로 둔다. 문구는 L3다. 소비자가 오류 **분류**를 기계로 읽는 자리(reason code, `CODE: …` 접두)는 L1이다.
- Python Traceback 흉내(`Traceback (most recent call last):` 31개 파일)는 없앤다. tripwire·소비자가 그 줄을 읽는 자리가 있으면 `consumers.md`가 L1로 적는다.
  manifest_contract 골든의 Traceback 행(Python 충돌, 종료 1)은 5절 처분을 따른다.
- `agent_runtime_support::failure::Failure { class, message }`의 `class`가 소비자에게 기계로 쓰이는 자리는 열거형 이름(문자열)을 L1로 남기고, 나머지는 크레이트 오류 타입으로 옮긴다.

## 4. JSON 값 모델 (PyValue·pyjson 대체)

- 동적 값: `serde_json::Value`(1.0.154, 기준 목록에 있음). 고정 스키마 부분은 `#[derive(Serialize, Deserialize)]` 구조체(serde 1.0.229·serde_derive 1.0.229, 기준 목록에 있음)로 둘 수 있다.
  단 아래 「영속 문서」 규칙을 지킨다.

### 영속 문서 (읽고 다시 쓰는 writer)

1. **모르는 키를 보존한다.** 문서를 읽어 고친 뒤 다시 쓰는 writer가 대상이다. 문서와 그 안의 객체는 `serde_json::Value`로 다루거나, 구조체에 `#[serde(flatten)] rest: serde_json::Map<String, Value>`를 둔다.
   그래서 모르는 키를 읽은 그대로 다시 쓴다. 아는 필드만 담는 구조체로 읽고 쓰기(모르는 키 버림)는 읽기 전용 소비자만 쓴다.
   - 근거: 원장 `.claude/policies/branch_layer_ledger.json` 항목 65개 중 52개 키가 스키마 밖 키다.
     - `_reach` 31은 `layer_ledger.rs:1121` `emit_skeleton`이 만든다. `_execution_note` 15, `_note` 5, `_approval_source` 1은 손으로 적은 키다.
     - upstream `layer_ledger`는 원장을 동적 값으로 읽는다(`layer_ledger.rs:919` `load`).
     - `append`(`:1021`)와 `merge`(`:1069`)는 파일 전체를 `save`(`:929-936`)로 다시 쓴다. 구조체로 옮기면 이 키가 조용히 사라진다.
   - 메인/서브 버전 혼재 정책 2(`consumers.md`)의 "읽는 쪽은 모르는 키를 무시한다"는 읽기 전용 소비자의 규칙이다. writer는 무시가 아니라 보존한다.
2. **값 없음(Python `None`)의 표기는 base를 따른다.** 지금 키를 `null`로 쓰는 자리는 `null`로 쓴다. 지금 키를 빼는 자리는 `#[serde(skip_serializing_if = "Option::is_none")]`로 뺀다.
   어느 쪽인지는 base 출력·골든이 정한다. 키 없음과 `null`은 파싱한 값이 다르므로 바꾸면 L2 값 변경(`surface-change`)이다.
3. **중복 키:** derive 구조체는 같은 필드가 두 번 나오면 `duplicate field` 오류를 낸다. `Value`(Map)는 뒤 값이 이긴다(`serde_json-1.0.154/src/value/de.rs:139-141`).
   지금 pyjson은 뒤 값이 이긴다. 단 `pyjson::loads_text_unique`를 쓰는 자리는 거부한다.
   - derive로 바꿔 전에 받던 문서를 거부하게 되면 divergence `output`(의미 `strictness`)이다.
   - `Value`로 읽는 자리에서 `loads_text_unique`와 같은 거부가 필요하면 사용자 정의 방문자를 둔다.
4. **writer 키 보존 계약 시험 의무:** 다시 쓰는 writer bin마다 시험을 둔다.
   - 모르는 키를 최상위와 항목 안에 넣은 입력으로 한 번 쓰게 하고, 출력에 그 키·값이 그대로 남는지 본다. 이전 cell은 이 시험을 빼지 않는다.
   - 대상 writer:
     - upstream `layer_ledger append`·`merge`(원장).
     - adversarial-benchmark `campaign_init`(캠페인 상태의 유일한 writer, workflow.md §캠페인).
     - terraforming `relay`(릴레이 원장 `campaigns/<id>/relay/<ctx>.json`).
     - policy-publisher `evidence_publisher append-raw`(발행 기록).
   - 시험은 그 크레이트의 c21 계열이 둔다(`consumers.md` 「계약 시험에 반영할 것」).

### 키 순서

- 출력 키 순서는 L2(자유)다. 다만 삽입 순서를 지키지 않으면 키가 정렬되어 모든 JSON 바이트 골든이 재기준선(divergence `layout`) 대상이 된다.
- 결정은 c30이 한다. 권고는 serde_json 기능 `preserve_order`다. 그 의존 `indexmap`은 기준 목록 밖이라 c30 추가안이고, `foldhash 0.2.0`은 목록에 있다.
- `preserve_order`를 켜면 다음을 지킨다.
  - `Map::remove`는 `swap_remove`와 같아서 마지막 항목을 지운 자리로 옮긴다(`serde_json-1.0.154/src/map.rs:160-166`). 순서를 지키려면 `shift_remove`를 쓴다.
  - 기능은 크레이트 빌드마다 합쳐진다. 그래서 16 크레이트가 같은 serde_json 기능 집합을 쓴다. serde_json을 직접 의존하는 소비 크레이트는 runtime-support와 같은 기능을 적는다.

### 입력

- RFC 8259 엄격이다. `NaN`·`Infinity`·`-Infinity`는 JSON이 아니므로 serde_json이 거부한다(지금 pyjson은 받는다).
  전에 받던 입력을 거부하게 되므로 divergence `output`(의미 `strictness`)이고, 거부 때 종료 2다.
- 중복 키를 거부하던 자리(`pyjson::loads_text_unique`)는 같은 거부를 유지한다(사용자 정의 방문자).
- **범위 밖 정수:** 정수는 `i64`·`u64` 범위다. 범위를 넘는 정수는 입력 오류다(pyjson의 BigInt·4300자리 상한은 버린다).
  - 그런데 serde_json은 u64를 넘는 정수와 i64 하한 밖 음수를 오류 없이 `f64`로 읽는다(`serde_json-1.0.154/src/de.rs:486-493` `parse_long_integer`). `-0`도 `f64` `-0.0`이 된다.
    그래서 기본 `Value` 읽기로는 이 규칙이 지켜지지 않는다.
  - 방문자만으로는 막지 못한다. 넘친 정수는 `visit_f64`로 와서 `1e20`과 구별되지 않는다.
  - c30 방법(권고): serde_json 기능 `arbitrary_precision`을 켠다. 의존 추가가 없고 serde_json 1.0.154는 기준 목록 안이다.
    - 읽은 뒤 수를 훑는다. 정수 모양(`-?[0-9]+`)인데 `as_i64`·`as_u64`가 모두 `None`이면 입력 오류(종료 2)다. `-0`은 정수 0이다(Python `json.loads`와 같다).
    - 실수 모양은 `as_f64`가 유한해야 한다(`1e400` 거부).
    - 이 기능을 켜면 `Number`가 원문 텍스트를 든다. 그래서 `1.0`과 `1.00`이 다른 값이 되므로 수 비교는 `as_f64`·`as_i64`로 한다.
    - 대안은 입력 바이트를 따로 훑는 토큰 검사다. c30이 정한다.
  - 지금 추적 JSON·JSONL 18파일·547토큰에 i64·u64 밖 정수가 있다. 모두 `checks/data`의 `*_api`·사례 데이터다.
    - adversarial-benchmark 11파일·515토큰: roofline_verdict `api_extra.jsonl` 272, guidellm `api_raw_cases.jsonl` 158 등.
    - recipe-runtime 7파일·32토큰.
    - 처분은 그 bin을 옮기는 cell(c44·c44b·c44c, c34)이 정한다. CPython BigInt 의미 시험이면 `retired`/`cpython-semantics`다.
      와이어 계약으로 남기면 typed wire(문자열 표기)로 바꾸고 소비자와 함께 `surface-change`다.

### 수·비유한 실수

- 정수는 `i64`/`u64`, 실수는 `f64`다. 기계 JSON에 비유한 실수를 쓰지 않는다.
- serde_json은 비유한 `f64`를 `null`로 쓴다. 지금 `NaN`·`Infinity` 토큰을 쓰는 자리는 값이 바뀐다(L2). 그래서 소비자와 같은 cell에서 정하고 divergence `fixed-surface`(의미 `surface-change`)로 올린다.
- 비유한 수의 생산자와 처분 cell. 셈은 JSON 값으로 쓰인 토큰만 센 것이다. 문자열 안의 "NaN" 글자까지 세는 grep은 55파일이다.

| 자리 | 근거 | 지금 | 처분 cell |
|---|---|---|---|
| terraforming `logs_lifecycle --apply`의 rollup | `logs_lifecycle.rs:158` `build_rollup` → `:395` `dumps_pretty`. parity 골든 `S5-04-apply`·`S5-04-apply-cap0`의 `files/n/rollup/2026-07-27.json`에 각 5토큰(`gpu_temp_c.max` = `Infinity`, `mem_avail_mib.{max,min,p01,p50}` = `NaN`) | 표본 CSV의 비유한 값이 통계로 나간다. 읽는 쪽은 `regen_envelope`(`days_from_rollup`) | c40-terraforming(읽는 쪽과 같은 cell, parity 재기준선은 `parity`) |
| terraforming의 손구현 JSON 직렬화 | `harness_permission_probe.rs:506-507`, `agent_control/pyhelp.rs:72-74` | 비유한 수를 `NaN`·`Infinity` 토큰으로 쓴다 | c40-terraforming |
| adversarial-benchmark `*_api` evaluate-jsonl 사례 데이터 | `checks/data` 23파일·1149토큰(guidellm 10파일, repeat_axis 3, roofline_verdict 2, lite_metrics 2, reporting 2, preset 2, dataset_prep 1, resolve_guidellm 1) | 와이어(`json-values-v1`)가 비유한 수를 나른다 | c44·c44b·c44c |
| recipe-runtime 사례 데이터 | `checks/data` 15파일·115토큰 | 같음 | c34-recipe-runtime |

- 합계는 40파일·1274토큰이다. 추적 `*.json`·`*.jsonl`을 `json.loads(parse_constant=…, parse_int=…)`로 읽어 셌다. 셈 스크립트와 파일 목록은 캠페인 c20 cell 폴더 `runs/r3-sweep/json_numbers_scan.*`에 있다(범위 밖 정수 셈도 같은 파일).

### 출력 형식

- **출력 형식**: 들여쓰기 2칸 pretty(`serde_json::to_writer_pretty`)는 Python `indent=2`와 같은 배치다. 기본 `json.dumps`(`", "`·`": "` 구분자)와
  `ensure_ascii=True`(비ASCII `\uXXXX`)는 바이트가 달라진다 — L2이므로 허용하고, 바이트 골든은 재기준선한다. 셸 소비자가 grep·sed하는 JSON 줄(L1, `consumers.md` 「텍스트로 읽는 JSON」)은 키와 값을 한 줄에 둔다.
- **float 표기**: Python `repr` 최단 왕복과 `serde_json`(ryu) 최단 왕복은 값이 같고 지수 표기만 다르다(`1e-05` 대 `1e-5`). L2.
- 지금 쓰기 함수 사용 수(소비 크레이트, `checks/` 밖): `dumps_ascii` 176, `dumps_pretty` 91, `dumps_unicode` 79, `dumps_indented` 13, `dumps_ascii_tight` 9, `json_dump_chunks` 4.

## 5. YAML 1.2 값 모델 (pyyaml·yaml11_oracle 대체)

- 파서: `yaml-rust2 = { version = "=0.13.0", default-features = false }`(기준 목록 h9 승인, D9 YAML 크레이트 선택). 결과는 아래 「Yaml→값 변환」(c30 한 곳)으로 4절의 JSON 값
  모델로 옮긴다. 하네스가 YAML 로더로 읽는 것은 manifest·토폴로지·캠페인·서빙 설정·bench preset, 그리고 compose(`output/<topo>/docker-compose.yaml`)다.
  모두 JSON 모양 문서다. compose는 terraforming `harness_permission_probe`가 읽는다(`harness_permission_probe.rs:190` `load_manifest` → `:214-216` `build_files`).
- 해소 규칙은 YAML 1.2 core schema다. PyYAML 1.1과 다른 점과 처리는 아래 표와 같다. yaml-rust2 열은 소스 근거다(`yaml-rust2-0.13.0/src/yaml.rs`, 줄은 그 파일 기준).
  W 캐시의 소스를 읽어 확인했고, 실행해 보지는 않았다. M 캐시에 이 크레이트가 아직 없기 때문이다. c30이 변환층 계약 시험으로 각 행을 실행해 확인한다.

| 구성 | yaml-rust2 0.13.0 | 변환층(c30) 규칙 | 계약 |
|---|---|---|---|
| `yes/Yes/YES/no/No/NO/on/On/ON/off/Off/OFF` | 문자열(`:704-716` `from_str`의 bool은 `true/True/TRUE/false/False/FALSE`만) | 그대로 문자열 | 1.1 bool 의존 입력은 `yaml_inputs.tsv` 처분대로 이전(c22·wd-yaml)하거나 퇴역(c60) |
| `True/TRUE/False/FALSE` | bool(`:706-707`) | 그대로 | 1.1과 값이 같다 |
| `Null`·`NULL` | **문자열**(`:705` null은 빈 값·`~`·`null`만. `!!null`도 `~`·`null`만 `:173-176`) | plain·태그 없는 `Null`·`NULL`을 Null로 본다(core schema·PyYAML과 같다) | 변환층이 맞춘다 |
| 앞자리 0 정수(`010`), `0o17`, `0x1A` | `010`은 10진 10, `0o`·`0x`는 정수(`:691-698`) | 그대로 | 1.1은 `010` = 8, `0o17` = 문자열. 하네스 로더 입력에서 0건 |
| `0b`·60진(`1:20`)·`_` 구분 수 | 문자열 | 그대로 | 하네스 로더 입력에서 0건 |
| i64 밖 정수(`100000000000000000000`, `0xfff…`) | 10진은 **Real**(i64 실패 → `parse_f64` 성공 `:709-713`), 16·8진은 **문자열**(`from_str_radix` 실패) | plain·태그 없는 스칼라가 core int 모양(`[-+]?[0-9]+`, `0o[0-7]+`, `0x[0-9a-fA-F]+`)인데 Integer가 아니면 다음과 같다. 양의 10진이 u64 안이면 u64이고, 아니면 입력 오류다 | strictness. 픽스처 G33·R48K04 |
| `.nan`·`.inf`·`-.inf`, `!!float nan` | Real(원문, `:62-72`·`:167-171`) | Real은 `f64`로 읽고, 비유한이면 입력 오류(4절 JSON 모델에 없다) | strictness. G31·G32·Y07·LM24 |
| 타임스탬프(`2026-01-01`) | 문자열 | 그대로 | date 타입(`Opaque`)을 버린다. 날짜 비교가 필요한 소비자는 문자열을 명시적으로 파싱한다 |
| 병합 키 `<<` | 일반 문자열 키(병합 없음). 한 매핑의 두 번째 `<<`는 중복 키 오류(`:217-228`) | **h15: 변환층이 병합한다**(아래 규칙). YAML 1.2 밖의 명시적 확장이다 | 지금 로더와 같은 값. compose 두 파일·LM09 |
| 중복 키 | `ScanError` "duplicated key in mapping"(`:217-227`) | 거부 그대로 | PyYAML은 뒤 값이 이긴다. 입력 오류(종료 2) — strictness. YT13 |
| 태그 | 스칼라는 다음과 같다. plain 스칼라의 `tag:yaml.org,2002:` `bool`·`int`·`float`·`null`만 해석하고, 그 밖의 접미(`binary`·`timestamp`·`python/…`)와 다른 핸들은 **문자열로 두고 태그를 버린다**(`:150-185`). 인용 스칼라는 태그와 상관없이 문자열이다(`:151`). 컬렉션 태그는 버린다(`:136-142` `SequenceStart(aid, _)`·`MappingStart(aid, _)`) — `!!set`·`!!omap`은 보통 매핑·시퀀스가 된다 | core 태그(`!!str`·`!!bool`·`!!int`·`!!float`·`!!null`·`!!seq`·`!!map`)만 받는다. 그 밖은 입력 오류다. 인용 스칼라에 `!!str` 밖 태그가 붙어도 입력 오류다. `Yaml`에는 태그가 남지 않으므로 c30은 `yaml_rust2::parser::MarkedEventReceiver`를 직접 구현한다 | strictness. python 태그는 지금도 거부라 보존된다(IO09·LM13). `!!binary`·`!!set`·`!!omap`은 퇴역 |
| 별칭·순환 | 별칭은 앵커 값의 복사본이다(`:189-196`). 앵커는 노드가 끝날 때 등록된다(`:202-205`). 그래서 자기 안을 가리키는 별칭은 `BadValue`다. 정의되지 않은 앵커는 파서 오류다(`parser.rs:676-681`) | `BadValue`·`Alias`가 하나라도 남으면 입력 오류 | PyYAML은 순환 구조를 만든다. strictness. YT15·YT16·R48K07·X10·LM22 |
| 문서 수 | `load_from_str` → `Vec<Yaml>`. 빈 입력·주석만은 문서 0개, `---`만은 `[Null]`, 여럿은 여럿이다 | 0개 → Null(PyYAML `safe_load`의 None), 1개 → 그 값, 2개 이상 → 입력 오류(PyYAML `ComposerError`와 같다) | 보존. IO06·IO07·LM03·LM04, LM10 |
| BOM | 맨 앞 U+FEFF를 벗기지 않는다(`scanner.rs:828` TODO). BOM 처리는 `encoding` 기능의 `YamlDecoder` 몫인데(`yaml.rs:392`) `default-features = false`라 없다 | 입력 맨 앞 U+FEFF 하나를 벗긴다(PyYAML 스캐너와 같다) | 보존. IO12·LM15 |
| 출력 불가 문자 | 스캐너가 c-printable을 검사하지 않는다(`char_traits.rs:114-122`는 emitter 쪽) | PyYAML Reader의 printable 집합 밖 문자는 입력 오류다. 대상은 탭·LF·CR을 뺀 C0, DEL, NEL을 뺀 C1, U+FFFE·U+FFFF다 | 보존. LM29 |
| 비문자열 키 | `Hash<Yaml, Yaml>`. 키가 정수·실수·bool·Null·시퀀스·매핑일 수 있다 | 키는 문자열만 받는다. 나머지는 입력 오류다. 문자열화하지 않는다(`1`과 `"1"`이 부딪친다) | strictness. R48K04·R48K08(LM21·Y01은 이미 퇴역) |
| 비UTF-8 | `load_from_str(&str)`이라 디코딩은 호출자 몫 | `std::str::from_utf8` 실패는 입력 오류 | 보존. IO11·X13·X14·LM14·LM27 |
| 문법 오류 | `ScanError` | 입력 오류 | 보존. IO08·IO10·QYa·QYb·R48K01·LM11·LM12. 같은 입력을 yaml-rust2도 거부하는지는 c31이 실행으로 확인하고, 다르면 divergence를 올린다 |

### Yaml→값 변환 (c30 한 곳)

runtime-support의 네이티브 YAML 층 하나가 다음을 한다. 소비 크레이트는 `yaml_rust2`를 직접 쓰지 않는다.

1. **입력:** 바이트를 UTF-8로 읽는다(실패 → 입력 오류). 맨 앞 U+FEFF 하나를 벗긴다. printable 집합 밖 문자는 입력 오류다.
2. **파싱:** `yaml_rust2::parser::Parser`와 c30의 `MarkedEventReceiver`를 쓴다. `YamlLoader`는 쓰지 않는다. 태그와 스칼라 스타일을 봐야 하고, 병합 키와 명시 중복 키를 가려야 하기 때문이다.
3. **문서 수:** 0개는 Null, 1개는 그 값, 2개 이상은 입력 오류다. 지금 로더의 "빈 문서 → `{}`"(`safe_load(…) or {}`, 예: `harness_permission_probe.rs:197`)는 소비자 쪽 규칙이므로 소비자가 지금처럼 처리한다.
4. **스칼라:**
   - 인용 스칼라는 문자열이다.
   - plain·태그 없는 스칼라는 yaml-rust2 `Yaml::from_str` 규칙을 따른다. 여기에 위 표의 `Null`·`NULL`과 i64 밖 정수 규칙을 더한다.
   - 태그는 core 태그만 받는다. `!!int`·`!!float`·`!!bool`·`!!null`이 값과 맞지 않으면(yaml-rust2의 `BadValue`) 입력 오류다.
5. **별칭:** 앵커 값의 복사본이다. 순환, 정의되지 않은 앵커, 남은 `BadValue`·`Alias`는 입력 오류다. 앵커를 다시 정의하면 그 뒤 별칭은 새 값을 가리킨다(yaml-rust2·PyYAML 같음).
6. **매핑:**
   - 키는 문자열만 받는다. 명시 키가 겹치면 입력 오류다.
   - 병합 키 `<<`는 h15에 따라 지금 로더와 같은 규칙으로 푼다. 지금 로더는 `runtime-support/src/pyyaml/compose.rs:927-1006` `flatten_mapping`이다.
     - 병합 키는 plain·태그 없는(또는 `!!merge`) 스칼라 `<<`다. 인용한 `"<<"`는 일반 키다.
     - 값이 매핑이면 그 매핑의 병합을 먼저 풀고, 그 항목을 병합 목록에 잇는다.
     - 값이 매핑의 시퀀스면 각 매핑을 풀어 **뒤에서 앞으로** 잇는다. 그 밖의 값(스칼라, 매핑이 아닌 원소)은 입력 오류다.
     - 한 매핑의 `<<`가 여럿이면 나온 차례로 모두 푼다.
     - 결과 항목은 "병합 목록 + 명시 항목" 순서다. 같은 키는 처음 나온 자리에 두고 값은 마지막 것이 이긴다. 그래서 명시 키가 병합 키를 이기고, 시퀀스 안에서는 앞 매핑이 뒤 매핑을 이긴다.
       병합으로 생긴 겹침은 중복 키 오류가 아니다.
     - 키 `=`(`!!value`)는 지금 로더처럼 문자열 키로 둔다.
7. **수:** 정수는 i64, 위 규칙의 u64를 쓴다. Real은 `f64`로 읽어 유한할 때만 `serde_json::Number::from_f64`로 옮기고, 그 밖은 입력 오류다.
8. **결과:** `serde_json::Value`다. 매핑 키 순서는 문서 순서다(키 순서 결정은 4절).
9. **오류:** 오류 값은 하나(종류·줄·열)다. 문구는 L3다. 소비자가 내는 종료 코드는 지금 그 소비자가 파싱 실패에 내는 코드다. 예를 들어 `manifest_contract`는 2(`manifest 파싱 실패`)이고, `_load_manifest` API는 예외를 낸다.

### 추적 manifest_contract 픽스처의 처분

- 대상은 manifest 로더(role-contract `load.rs:70`)로 읽히는 adversarial-benchmark 픽스처다.
  - `checks/data/manifest_contract/manifests/<ID>.yaml`(CLI 골든 `original190.json`·`extra9.json`·`extra14.json`·`deep2.json`·`key4.json`·`key8.json`·`gap3.json`·`bytedepth.json`의 `exit`).
  - `checks/data/manifest_contract_api/manifests/<ID>__*.yaml`(API 골든 `api87.json`의 `RETURNED`·`RAISED`).
- 로더의 파서를 바꾸는 cell(c31-role-contract)이 같은 cell에서 아래처럼 한다. 퇴역 행의 원장 기록은 c60이 한다.
  - 보존 행은 기대값을 그대로 둔다.
  - 퇴역 행은 골든 행을 빼거나 기대 종료 코드를 2로 바꾸고, divergence `output`(의미 `strictness`)와 `test_delta.tsv`를 올린다.
- 처분 결과(`yaml_inputs.tsv` `disposition`): 변환층 보존 7, 변환층 거부로 base 종료 코드 보존 16, 퇴역 21이다. 이미 퇴역이던 10개는 그대로다.

| 처분 | 픽스처(base 기대) | 이유 |
|---|---|---|
| 손대지 않음 — 변환층 보존 | IO06·IO07(종료 4), LM03·LM04(`{}`) | 문서 0개 → Null, 소비자의 `or {}` |
| 손대지 않음 — 변환층 보존 | IO12(종료 0), LM15(값) | BOM 하나를 벗긴다 |
| 손대지 않음 — 변환층 보존 | LM09(병합된 값) | h15 병합. 1회차의 퇴역에서 뺐다 |
| 손대지 않음 — 변환층 거부로 base 종료 코드 보존 | IO09(종료 2), LM13(`RAISED`) | core 밖 태그(python) 거부 |
| 손대지 않음 — 변환층 거부로 base 종료 코드 보존 | LM10(`RAISED`) | 문서 2개 이상 거부 |
| 손대지 않음 — 변환층 거부로 base 종료 코드 보존 | LM29(`RAISED`) | printable 집합 밖 문자(`"\x01"`) 거부 |
| 손대지 않음 — 변환층 거부로 base 종료 코드 보존 | IO11·X13·X14(종료 2), LM14·LM27(`RAISED`) | 비UTF-8 거부 |
| 손대지 않음 — 변환층 거부로 base 종료 코드 보존 | IO08·IO10·QYa·QYb·R48K01(종료 2), LM11·LM12(`RAISED`) | 문법 오류. c31이 실행으로 확인 |
| 퇴역 | Y03·Y04·Y06·YT20·R48K05(종료 1, Traceback), Y05(종료 0) | `!!binary` 거부 → 종료 2 |
| 퇴역 | YT21(종료 1), YT22(종료 0) | `!!set`·`!!omap` 거부 |
| 퇴역 | G31(종료 1), G32(종료 5), Y07(종료 0), LM24(값) | `.nan`·`.inf` 거부 |
| 퇴역 | G33(종료 0) | 1e20 정수, i64·u64 밖 거부 |
| 퇴역 | R48K04(종료 1, `0xfff…` 키), R48K08(종료 1, 실수 키) | 비문자열·범위 밖 키 거부 |
| 퇴역 | YT15(종료 0), YT16·R48K07·X10(종료 1), LM22(값) | 순환 별칭 거부 |
| 퇴역 | YT13(종료 6, 뒤 `topology: multi`가 이김) | 중복 키 거부 |
| 퇴역(1회차 그대로) | X05~X09·Y01·YT17~YT19·LM21 | 타임스탬프·`yes` 등 1.1 해소 |

- 퇴역의 근거는 다음과 같다.
  - base 동작이 Python 충돌(Traceback, 종료 1)이거나, PyYAML만의 값(bytes·set·tuple·NaN·BigInt·순환)을 통과시킨 것이다.
  - 같은 성질은 c30 변환층 계약 시험(태그·비유한·범위 밖 정수·순환·중복 키 거부)과 보존 행 IO09가 지킨다.
- 셈 방법: PyYAML 6.0.3 `compose_all`로 노드(태그·스타일·키)를 훑었다. 대상은 `yaml_inputs.tsv`에서 `yaml_loader`가 yes인 추적 282개와 비추적 216개다(오케스트레이터 스냅샷).
  스크립트와 결과는 캠페인 c20 cell 폴더 `runs/r3-sweep/yaml_conv_scan.*`에 있다. compose 두 파일은 이 셈 뒤에 `yaml_loader`를 yes로 고쳤다.
  비추적 216개(manifest 4·캠페인 54·서빙 설정 157·bench preset 1)는 위 규칙에 하나도 걸리지 않는다. 추적 compose 두 파일은 병합 키만 쓴다.

### 그 밖의 YAML

- YAML 쓰기(지금 `pyyaml::dump`는 recipe `simlog.rs:735`의 `trialNN_candidate.yaml` 하나뿐)는 `yaml_rust2::YamlEmitter`로 옮긴다. 그 파일은 사람용 산출물이며
  하네스가 다시 읽지 않는다(`recipe_cmd/simulate_body.rs:194`). 바이트 차이는 `golden`.
- `wiki-desk/rust`의 `yaml11_oracle`(PyYAML 6.0.1 해소 손 재현)은 c45가 "하네스 YAML 파서로 문자열 왕복" 검사로 바꾼다(plan c45). frontmatter.check.ts의 기대값도 그때 고친다.
  wiki-desk `checks/contract_checks.rs:60-79`가 지금 고정한 PyYAML 1.1 의미(`yes` → bool, `017`, 타임스탬프, 중복 키 뒤 값)도 그때 바꾼다(검토 B-B4).
- 오류: `ScanError`의 위치(줄·열)는 사람용 메시지(L3)로만 쓴다. PyYAML 문구·Traceback·`RecursionError` 흉내(`pyyaml/frames.rs`, `sites.rs`)는 버린다.

### yaml_inputs.tsv (YAML 1.1 의존 입력 인벤토리)

- 범위: M의 추적 YAML 355개(`git ls-files '*.yaml' '*.yml'`)와 메인 비추적 YAML 스냅샷 817개(오케스트레이터 스냅샷, 캠페인 c20 `bundle/w-untracked-yaml/`), 서브는 한 행(서브 몫, wd-yaml).
- 열: `scope`(tracked|untracked|sub), `path`, `kind`(manifest·campaign·serving-config·bench-preset·certificate·compose·fixture·simlog·third-party 등),
  `tokens16`(FORMAT.md 정의: 구 파서 `is_bool` 18개 중 `true`·`false`가 아닌 16개가 plain scalar로 쓰인 수, 키 포함), `bool_change`(그중 1.2에서 문자열이 되는
  `yes/no/on/off` 12개 철자의 수), `other_change`(그 밖에 1.1 해소와 1.2 core schema 해소가 다른 plain scalar 수: 타임스탬프·병합 키·앞자리 0 수 등), `detail`, `parse`(1.1 파서로
  토큰화한 결과: ok·yaml-error·not-utf8), `reader`(근거 경로), `yaml_loader`(하네스 YAML 로더가 읽으면 yes), `disposition`(이전|퇴역|손대지 않음|서브 몫), `owner_cell`, `reason`.
- 셈 방법: 1.1 쪽은 PyYAML SafeLoader 토크나이저·해소기, 1.2 쪽은 core schema 정규식(yaml-rust2의 해소와 같은 규칙)으로 각 plain scalar를 해석해 비교했다. 따옴표 친 값은 세지 않는다.
- 결과:
  - 메인 비추적 817개 중 하네스 YAML 로더가 읽는 216개(manifest 4, 캠페인 54, 서빙 설정 157, bench preset 1)에 1.1 의존 토큰이 **0**이다.
  - 16토큰이 있는 75개는 인증서 30(flat 파서, 4절 위), 외부 크레이트 CI 설정 43, parity 원시 1, 서브 회수 미러 1이다.
  - 따라서 메인의 이전 대상은 없다. h11로 결정됐다: wd-yaml 메인 몫은 이전 0이고, 하네스가 YAML로 읽는 216개가 구·신 파서에서 같은 값인지 `yaml_compat compare3`로 대조만 한다. 서브 몫은 plan대로다.
  - 추적 입력의 퇴역은 31개다(위 「추적 manifest_contract 픽스처의 처분」). compose 두 파일(`output/{single,multi}/docker-compose.yaml`)은 하네스 YAML 로더가 읽는다(`yaml_loader` yes). h15 병합으로 손대지 않는다.

## 6. py* 계층 대체 대응표

| 지금 (runtime-support) | 줄 수 | 대체 | 시점 |
|---|---:|---|---|
| `pystr::PyStr`(u32 코드 포인트, surrogateescape) | 549 | `String`·`&str`, 경로는 `PathBuf` (2절) | c30에서 네이티브 계층 추가, 소비자 이전 cell에서 교체 |
| `pyvalue::{PyValue, PyDict, DictKey, Opaque}` + JSON 쓰기 | 1836 | `serde_json::Value`(+ `preserve_order`) 또는 serde 구조체 | 같음 |
| `pyjson`(CPython `_json.c` 이식 읽기) | 1236 | `serde_json::from_str`·`from_slice` (4절) | 같음 |
| `pyyaml/*`(PyYAML SafeLoader·dump 이식, `sites.rs` 생성 표 포함) | 12193 | `yaml-rust2` (5절) | c30 추가, c22 대조 도구, 소비자 이전 cell에서 교체 |
| `pynumber`(int/float/round, BigInt) | 762 | `i64`/`u64`/`f64`, 표준 `parse`, 반올림은 필요한 자리에서 명시 | 소비자 이전 cell |
| `pyformat`(`str % float`) | 357 | `format!` | 같음 |
| `pysort`(powersort 비교 순서 재현) | 734 | `slice::sort_by`(안정 정렬)·`Ord` 구현 | 같음 |
| `pyops`(bool/int/float 혼합 산술) | 217 | 타입이 정해진 산술 | 같음 |
| `pycall`(Python 호출 바인딩) | 199 | 함수 시그니처 | 같음 |
| `pydyn`(동적 첨자·반복·join) | 447 | 타입이 정해진 접근(`Value::get`, 구조체 필드) | 같음 |
| `pylower`·`unicode_*_table.rs`(Unicode 15 동결 표) | 113+3611 | 표준 `char` 메서드 | 같음 |
| `failure::Failure` | 46 | 크레이트별 오류 enum (3절) | 같음 |
| `schema`(Draft-07 부분 검증기, PyValue 위) | 280 | 같은 검증을 `serde_json::Value` 위로 옮긴다(새 크레이트 없이) | c32(policy-gate) |

- c30은 py*를 지우지 않고 옆에 네이티브 계층(텍스트·값·JSON·YAML)을 더한다. 소비자 이전 동안 `PyValue` ↔ 네이티브 값 변환(브리지)을 runtime-support 한 곳에 둔다.
  브리지와 py*는 c60이 빌드에서 빼고 `delete.txt`에 적는다.
- **브리지 방향(손실 변환은 두지 않는다):**
  - `Value → PyValue`는 전체 `From`이다. 실패가 없다.
    - Null → None, Bool → Bool, Number(i64·u64) → Int, Number(f64) → Float, String → Str, Array → List.
    - Object → Dict(문자열 키, 순서 유지).
  - `PyValue → Value`는 `TryFrom`이다. 실패하는 값은 다음과 같다.
    - Int가 i64·u64 밖, Float가 비유한, Str에 홀 서로게이트(UTF-8이 아님).
    - Dict에 문자열이 아닌 키(`DictKey::Other`).
    - `Bytes`·`Tuple`·`Opaque`, 그리고 순환하는 `Shared`. Tuple은 `!!omap`·`!!pairs`만 만들고 변환층이 거부한다.
  - 실패는 그 소비자의 입력 오류로 보고한다(종료 2. 오류 분류가 L1인 자리는 지금 reason code). 문자열화, `null`, `to_string_lossy`, f64 반올림으로 대신하지 않는다. runtime-support `#[test]` 85개 + `checks/pycall_cases.rs` 6개는 모두 py* 쪽이므로 c60 `test_delta.tsv` 대상이다.
- 상수 문자열 매크로 `lit!`(호출 자리마다 버퍼 하나를 누수)은 `&'static str`로 바꾼다.

## 7. 손구현 사본 → 커뮤니티 크레이트

| 사본 | 위치 | 줄 | 대체 | cell |
|---|---|---:|---|---|
| argparse 3.12 바이트 재현 `cliparse` | recipe-runtime `src/cliparse.rs` (upstream 34·policy-predicates 1 파일이 import) | 836 | clap(derive) | c34, upstream 쪽은 c43 |
| argparse 사본 | policy-audit `src/argparse.rs` 555 · policy-gate `src/argp.rs` 247 · policy-selftest `src/argp.rs` 178 · hint-publisher `src/cli/argparse.rs` 892 · terraforming `src/argp.rs` 609 | — | clap | c50·c32·c51·c33·c40 |
| argv 파서 | adversarial-benchmark `src/argv.rs` 1794 · upstream `src/runtime_patch_args.rs` 353 | — | clap | c44·c43 |
| fnmatch/glob | recipe `src/model_files.rs` 865 · policy-distribution `src/fsglob.rs` 79 · adversarial-benchmark `src/ospath.rs` 587 · policy-selftest `src/tripwires.rs:513` · adversarial-benchmark `src/campaign_write.rs:3532` | — | globset(+ walkdir 또는 표준 `read_dir` 재귀) | c34·c42·c44·c51 |
| Python `re` 엔진 | recipe `src/vllm_log.rs` 688 · upstream `src/py_regex_engine.rs`+`py_regex_parse.rs` 2526 · upstream `src/requirement/pyre.rs` 673 · terraforming `src/pyre.rs` 406 · hint-publisher `src/pyre.rs` 207 | — | regex(lookaround가 꼭 필요한 패턴만 fancy-regex 유지) | c34·c43·c40·c33 |
| YAML 1.1 해소 | wiki-desk `rust/src/main.rs`(`yaml11_oracle`) | 352 | yaml-rust2 왕복 검사 | c45 |

- clap·globset·regex·walkdir·indexmap은 기준 허용 목록에 넣지 않았다. 이 노드의 오프라인 캐시(M·W 모두)에 없어 정확한 판을 정할 수 없기 때문이다.
  쓰는 cell이 cell별 추가안으로 판을 올리고 사람 승인(D9)을 받는다. regex 퍼사드는 이미 고정된 `regex-automata 0.4.18`·`regex-syntax 0.8.11`과 맞는 판을 고른다.
  - 목록 밖 크레이트의 M 쪽 최초 온라인 fetch는 오케스트레이터가 한다. 빌더 cell은 오프라인(`--offline`)으로만 빌드한다.
  - W 쪽 fetch는 배달 때 워커가 한다. 별도 실행 기록을 남긴다(D2·D11 미러 캐시).
  - yaml-rust2와 그 의존 4개도 같은 규칙이다. c30 전에 오케스트레이터가 M에서 fetch한다(h9).
- 다른 Python 의미 사본(policy-audit `pydate.rs`·`pyids.rs`, `pytext.rs` 4벌, role-contract `pyprofile.rs`, recipe `pyexpr.rs`, policy-selftest `pyast.rs`·`pjson.rs`,
  adversarial-benchmark `pyfs.rs`)도 py* 삭제 범위의 grep(`pyfree`)과 별도로 각 크레이트 cell이 정리한다. `pyfree` 검사의 정규식(`PyStr|py(call|dyn|…|yaml)`)은 이 이름들을 잡지 않는다.

## 8. 이전 순서 (path 소비자 12개)

path 의존 층(Cargo.toml `path =` 기준, 아래가 먼저):

| 층 | 크레이트 | 묶음 |
|---|---|---|
| L0 | runtime-support (독립: parity, py-boundary-guard, wiki-desk-oracle) | g-rs c30·c22 |
| L1 | role-contract, policy-gate, hint-publisher, recipe-runtime | g-t1 c31~c34 |
| L2 | terraforming(← role-contract), policy-publisher(← policy-gate), policy-distribution(← hint-publisher), upstream-runtime(← role-contract·recipe·hint) | g-t2 c40~c43 |
| L3 | adversarial-benchmark(← role-contract·policy-gate·**policy-publisher**·hint·recipe), policy-audit(← upstream-runtime) | g-t2 c44, g-t3 c50 |
| L4 | policy-selftest(← role-contract·policy-gate·policy-audit·policy-distribution·hint) | g-t3 c51 |
| L5 | policy-predicates(← 거의 전부) | g-t3 c52 |

- g-t2 안에서 c44(adversarial-benchmark)는 policy-publisher(c41)에 path로 의존한다. c41이 공개 API의 py 타입을 바꾸면 c44가 뒤에 와야 한다(같은 묶음 needs).
- py 타입이 크레이트 경계를 넘는 자리(이전 때 같이 끊는다):
  upstream-runtime → `recipe_runtime::cliparse`(34 파일)·`recipe_runtime::model_files`(`layout_files.rs:26`, `manifest.rs:13`),
  adversarial-benchmark의 재수출 `pub use agent_runtime_support::{pydyn,…,pyyaml}`(`src/lib.rs:101-111`)과 그것을 쓰는 policy-predicates(`claims_hint.rs:1244`, `claims_tf.rs:157`),
  policy-distribution의 `pub use PyDict, PyValue`(`src/lib.rs:14`), policy-gate의 `schema::validate_against_schema` 재수출(`src/completion_gate.rs:16`).
- 한 묶음 안에서 하위 크레이트의 공개 API를 네이티브 타입으로 바꾸면, 상위 소비자는 브리지(6절)로 잠시 잇는다. 브리지는 c60에서 사라진다.
- 소비 크레이트별 py* 사용 파일 수(`.rs`, src+checks): sweep-stop 322 중 PyStr 192·PyValue 202, upstream 210 중 117·90, recipe 150 중 110·92, terraforming 86 중 27·33,
  role-contract 26 중 19·20, policy-predicates 23 중 4·12, policy-selftest 20 중 4·5, policy-audit 17 중 3·7, policy-gate·publisher·distribution 각 11 중 1~3·4~6,
  hint-publisher 66 중 2·1(자체 값 타입 `J`가 있어 거의 네이티브).

## 9. 알려진 불일치

- runtime-support `README.md`는 "47 unit tests"라고 적지만 `#[test]`는 85개다(+ `checks/` 6개).
- `upstream-version-watch/rust/runtime/src/native_multinode/up.rs:4`의 모듈 문서는 "`blackbox_session.py` … Python that remains"라고 적지만, 세션 도구는 이미 Rust 런처다(`consumers.md` 「세션 도구」). 문서 줄만 낡았다.
- `pyyaml/mod.rs:1`은 PyYAML 6.0.3, `pyyaml/sites.rs:1`과 wiki-desk 오라클은 6.0.1이라고 적는다.
- `docs/benchmark/benchmark_*.yaml`(메인 92개, 그중 30개에 `e_search: no`, 3개에 `*_backend_mismatch: no`)은 YAML 로더가 아니라 flat 문자열 파서(`completion_gate.rs:499`)가 읽는다.
  YAML 1.1 bool 의존 입력이 아니다(`yaml_inputs.tsv`).
