# 네이티브 타입 계약 (runtime-support · 16 크레이트 공통)

- 정본: plan 26111805 「보존 계약」·「cell 세부」 c20, interview D1·D3·D4·D7·D8·D9·D16·D17.
- 쓴 cell: c20-contract (base `ead7087d`). 읽는 cell: c21(계약 시험), c22(yaml_compat), c30~c60(이전·삭제).
- 같은 폴더: `consumers.tsv`·`consumers.md`(고정 표면과 소비자), `entrypoints.tsv`(결정론 진입점),
  `yaml_inputs.tsv`(YAML 1.1 의존 입력), `divergence_kinds.md`(divergence 의미 분류 제안).
- 크레이트 판·라이선스는 `.claude/rust/py-boundary-guard/crate_allowlist.tsv`가 정본이다. 이 문서는 어떤 크레이트를 어디에 쓰는지만 적는다.
- 이 문서의 결정은 c20 검토(judgment)로 수용한다. 크레이트 판·라이선스는 h9로 승인됐다(`crate_allowlist.tsv` 85행과 허용 라이선스 집합).
  바꾸려면 c20을 다시 연다(c21도 다시 연다).

## 1. 보존 기준과 출력 호환 수준

rust-repack의 기본값 "외부 동작 보존"을 **고정 표면**(D8)으로 한정한다. CPython 의미(문자열 = 코드 포인트, `repr`, `json.dumps` 바이트,
PyYAML 1.1 해소, argparse 문구, Traceback 흉내)는 보존 대상이 아니다(D1·D3).

| 수준 | 대상 | 이전 뒤 요구 |
|---|---|---|
| **L1 바이트 고정** | 종료 코드 값, 표식 줄(`…-PASS` 등), reason_codes 문자열, 기계 파싱 표(SKILL.md §4 공개 표면 표 등), tripwire가 읽는 stderr 줄, 셸 소비자가 grep·awk로 읽는 줄, 서브커맨드·플래그 long 이름과 실제 쓰는 짧은 형, 런처 환경 변수 이름 | 같은 바이트. 바꾸면 같은 cell에서 소비자를 고치고 divergence `fixed-surface` 행 |
| **L2 값 고정** | JSON 파서가 읽는 stdout·파일(키, JSON 타입, 값), stdout/stderr 분리 | 파싱한 값이 같다. 키 순서·공백·이스케이프·float 표기는 자유. 바이트 비교 골든은 재기준선(divergence `golden`·`parity`) |
| **L3 자유** | usage·help·오류 문구와 배치, 접두어 축약, Python repr·Traceback 흉내, 진단 메시지 | 소비자가 없으면 자유. 바뀐 골든은 `golden` 재기준선 |

- 어느 출력이 L1·L2인지는 `consumers.tsv`의 `reads` 열이 정한다. 셸 소비자가 JSON 출력을 텍스트로 grep하면 그 줄은 L1이다.
- 접두어 축약(argparse의 `--rep` → `--repo`)은 자유다. 저장소 안 소비자가 축약형을 쓰면(`consumers.tsv` `invocation`) 그 형을 명시 별칭으로 남긴다.

## 2. 텍스트와 OS 문자열

| 값 | 타입 | 비UTF-8을 만나면 |
|---|---|---|
| 기계 텍스트(JSON·YAML 값, reason code, 메시지, manifest 값) | `String`·`&str` | 경계에서 오류(입력 오류 → 종료 2). surrogateescape로 숨겨 나르지 않는다 |
| 파일 경로·파일명 | `PathBuf`·`&Path`(`OsString` 기반) | 그대로 나른다. `String`으로 바꾸지 않는다 |
| 경로를 기계 출력(JSON 값·키, L1 줄)에 쓸 때 | `path.to_str()` | `None`이면 오류(입력 오류 → 종료 2). `to_string_lossy`로 U+FFFD를 섞어 내지 않는다 |
| 경로를 사람용 메시지에 쓸 때 | `path.display()` | 손실 표시 허용(L3) |
| argv | `std::env::args_os()` → clap(`value_parser!(PathBuf)`는 경로, `String`은 텍스트) | 텍스트 인자의 비UTF-8은 clap 사용법 오류(종료 2). `std::env::args()`는 쓰지 않는다(비UTF-8에서 panic) |
| 환경 변수(경로) | `std::env::var_os` | 그대로 `PathBuf` |
| 환경 변수(텍스트) | `std::env::var` | `Err(NotUnicode)`는 오류로 보고한다. 없음(`NotPresent`)과 섞지 않는다 |
| 바이트 데이터(해시 대상, 로그 원문) | `Vec<u8>`·`&[u8]` | 해당 없음 |

- 지금은 세 관례가 섞여 있다: surrogateescape `PyStr`(sweep-stop 41·recipe 17·upstream 10 파일), `to_string_lossy`(policy-gate·policy-selftest·terraforming),
  `std::env::args()`(policy-audit·distribution·publisher·predicates·hint-publisher·parity·py-boundary-guard와 `*_api`·`*_bench` bin들). 이전한 크레이트는 위 표 하나로 맞춘다.
- `PyStr`이 담던 홀 서로게이트(U+D800..DFFF)는 `String`에 없다. 저장소의 입력(manifest·원장·캠페인 상태·argv)에서 홀 서로게이트를 일부러 나르는 계약은 찾지 못했다.
  c21 계약 시험은 비UTF-8 argv 한 사례(종료 2)와 비UTF-8 파일명 한 사례(경로로 나르고, JSON에는 쓰지 않거나 종료 2)를 고정한다.
- 대소문자·공백 판정은 Rust 표준(`char::is_whitespace`, `to_lowercase`)을 쓴다. CPython 3.12 Unicode 15 표(`pylower`, `unicode_*_table.rs`)는 버린다.
  비교 결과가 달라지는 자리는 divergence `output`·`cpython-semantics`다.

## 3. 오류와 종료 코드

- `main`은 `std::process::ExitCode`를 돌려준다. 종료 코드는 `u8` 범위의 값으로만 만든다. `SystemExit` 흉내(`& 0xff` 대 `i32::try_from`의 갈림)는 버리고,
  bin마다 실제로 쓰는 코드 집합을 c21이 고정한다(`consumers.md` 「종료 코드」).
- 공통 의미(지금 관례 그대로): 0 통과·성공, 1 검사 실패·위반, 2 사용법 또는 입력 오류(파싱 불가, 읽을 수 없는 파일, 비UTF-8 텍스트). 그 밖의 코드(예: lite/sweep 6·7)는 bin별 고정 표면이다.
- 오류 값은 크레이트마다 `enum`(+ `std::error::Error`·`Display`)으로 둔다. 문구는 L3다. 소비자가 오류 **분류**를 기계로 읽는 자리(reason code, `CODE: …` 접두)는 L1이다.
- Python Traceback 흉내(`Traceback (most recent call last):` 31개 파일)는 없앤다. tripwire·소비자가 그 줄을 읽는 자리가 있으면 `consumers.md`가 L1로 적는다.
- `agent_runtime_support::failure::Failure { class, message }`의 `class`가 소비자에게 기계로 쓰이는 자리는 열거형 이름(문자열)을 L1로 남기고, 나머지는 크레이트 오류 타입으로 옮긴다.

## 4. JSON 값 모델 (PyValue·pyjson 대체)

- 동적 값: `serde_json::Value`(1.0.154, 기준 목록에 있음). 스키마가 고정된 문서(원장 행, 캠페인 상태, receipt, 결과 JSON)는 `#[derive(Serialize, Deserialize)]` 구조체로 읽고 쓴다
  (serde 1.0.229·serde_derive 1.0.229, 기준 목록에 있음).
- **키 순서**: 출력은 삽입 순서를 지킨다(`serde_json` 기능 `preserve_order`, 의존 `indexmap` — 기준 목록 밖, c30 cell 추가안). 쓰지 않으면 키가 정렬되어 모든 JSON 골든이
  재기준선 대상이 된다. 어느 쪽이든 L2이므로 소비자 계약은 깨지지 않는다. 권고: `preserve_order`.
- **입력**: RFC 8259 엄격. `NaN`·`Infinity`·`-Infinity`는 거부한다(지금 pyjson은 받는다). 지금 이 상수를 입력으로 주고받는 자리는 c21이 계약 시험으로 찾고,
  바꾸는 cell이 divergence `fixed-surface`(소비자 갱신과 함께)로 올린다. 중복 키를 거부하던 자리(`pyjson::loads_text_unique`)는 같은 거부를 유지한다(사용자 정의 방문자).
- **수**: 정수는 `i64`/`u64`. 범위를 넘는 정수는 입력 오류다(pyjson의 BigInt·4300자리 상한은 버린다). 실수는 `f64`. 기계 JSON에 비유한 실수를 쓰지 않는다
  (`serde_json`은 NaN을 `null`로 쓴다 — 그런 자리는 값 변경이므로 divergence `fixed-surface`).
- **출력 형식**: 들여쓰기 2칸 pretty(`serde_json::to_writer_pretty`)는 Python `indent=2`와 같은 배치다. 기본 `json.dumps`(`", "`·`": "` 구분자)와
  `ensure_ascii=True`(비ASCII `\uXXXX`)는 바이트가 달라진다 — L2이므로 허용하고, 바이트 골든은 재기준선한다. 셸 소비자가 grep하는 JSON 줄(L1)은 c21이 찾아 고정한다.
- **float 표기**: Python `repr` 최단 왕복과 `serde_json`(ryu) 최단 왕복은 값이 같고 지수 표기만 다르다(`1e-05` 대 `1e-5`). L2.
- 지금 쓰기 함수 사용 수(소비 크레이트, `checks/` 밖): `dumps_ascii` 176, `dumps_pretty` 91, `dumps_unicode` 79, `dumps_indented` 13, `dumps_ascii_tight` 9, `json_dump_chunks` 4.

## 5. YAML 1.2 값 모델 (pyyaml·yaml11_oracle 대체)

- 파서: `yaml-rust2 = { version = "=0.13.0", default-features = false }`(기준 목록 h9 승인, D9 YAML 크레이트 선택). 결과 `yaml_rust2::Yaml`은 경계에서 4절의 JSON 값
  모델(또는 구조체)로 옮긴다. 하네스가 YAML에서 읽는 것은 manifest·토폴로지·캠페인·서빙 설정·bench preset으로, 모두 JSON 모양 문서다.
- 해소 규칙은 YAML 1.2 core schema다. PyYAML 1.1과 다른 점과 처리:

| 1.1 구성 | yaml-rust2 0.13.0 | 계약 |
|---|---|---|
| `yes/Yes/YES/no/No/NO/on/On/ON/off/Off/OFF` | 문자열 | 1.1 bool 의존 입력은 `yaml_inputs.tsv` 처분대로 이전(c22·wd-yaml)하거나 퇴역(c60) |
| `True/TRUE/False/FALSE` | bool(1.1과 같음) | 의미 변화 없음. FORMAT.md의 이전 토큰 16개에는 들어 있으나 이전해도 값이 같다 |
| 앞자리 0 정수(`010`), `0b`, 60진(`1:20`), `_` 구분 수 | 10진 정수 또는 문자열 | 의미가 달라진다. 하네스 로더 입력에서 0건(`yaml_inputs.tsv` `other_change`) |
| 타임스탬프(`2026-01-01`) | 문자열 | date 타입(`Opaque`)을 버린다. 날짜 비교가 필요한 소비자는 문자열을 명시적으로 파싱한다 |
| 병합 키 `<<` | 일반 키(병합하지 않음) | 하네스 YAML 로더 입력에는 쓰지 않는다. compose 파일은 docker compose가 읽고 하네스는 정규식으로만 읽는다(`yaml_inputs.tsv` kind `compose`) |
| 중복 키 | **오류** | PyYAML은 뒤 값이 이긴다. 중복 키 입력은 입력 오류(종료 2)로 바뀐다 — divergence `output` |
| `!!binary`·`!!set`·`!!omap` | 지원 안 함 | 하네스 입력에서 쓰지 않는다. 만나면 입력 오류 |
| 별칭(`*a`) | 앵커 값으로 풀림 | 순환 문서는 입력 오류(PyYAML의 `[...]` 표지는 버린다) |

- YAML 쓰기(지금 `pyyaml::dump`는 recipe `simlog.rs:735`의 `trialNN_candidate.yaml` 하나뿐)는 `yaml_rust2::YamlEmitter`로 옮긴다. 그 파일은 사람용 산출물이며
  하네스가 다시 읽지 않는다(`recipe_cmd/simulate_body.rs:194`). 바이트 차이는 `golden`.
- `wiki-desk/rust`의 `yaml11_oracle`(PyYAML 6.0.1 해소 손 재현)은 c45가 "하네스 YAML 파서로 문자열 왕복" 검사로 바꾼다(plan c45). frontmatter.check.ts의 기대값도 그때 고친다.
- 오류: `ScanError`의 위치(줄·열)는 사람용 메시지(L3)로만 쓴다. PyYAML 문구·Traceback·`RecursionError` 흉내(`pyyaml/frames.rs`, `sites.rs`)는 버린다.

### yaml_inputs.tsv (YAML 1.1 의존 입력 인벤토리)

- 범위: M의 추적 YAML 355개(`git ls-files '*.yaml' '*.yml'`)와 메인 비추적 YAML 스냅샷 817개(오케스트레이터 스냅샷, 캠페인 c20 `bundle/w-untracked-yaml/`), 서브는 한 행(서브 몫, wd-yaml).
- 열: `scope`(tracked|untracked|sub), `path`, `kind`(manifest·campaign·serving-config·bench-preset·certificate·compose·fixture·simlog·third-party 등),
  `tokens16`(FORMAT.md 정의: 구 파서 `is_bool` 18개 중 `true`·`false`가 아닌 16개가 plain scalar로 쓰인 수, 키 포함), `bool_change`(그중 1.2에서 문자열이 되는
  `yes/no/on/off` 12개 철자의 수), `other_change`(그 밖에 1.1 해소와 1.2 core schema 해소가 다른 plain scalar 수: 타임스탬프·병합 키·앞자리 0 수 등), `detail`, `parse`(1.1 파서로
  토큰화한 결과: ok·yaml-error·not-utf8), `reader`(근거 경로), `yaml_loader`(하네스 YAML 로더가 읽으면 yes), `disposition`(이전|퇴역|손대지 않음|서브 몫), `owner_cell`, `reason`.
- 셈 방법: 1.1 쪽은 PyYAML SafeLoader 토크나이저·해소기, 1.2 쪽은 core schema 정규식(yaml-rust2의 해소와 같은 규칙)으로 각 plain scalar를 해석해 비교했다. 따옴표 친 값은 세지 않는다.
- 결과: 메인 비추적 817개 중 하네스 YAML 로더가 읽는 216개(manifest 4, 캠페인 54, 서빙 설정 157, bench preset 1)에 1.1 의존 토큰이 **0**이다. 16토큰이 있는 75개는
  인증서 30(flat 파서, 4절 위), 외부 크레이트 CI 설정 43, parity 원시 1, 서브 회수 미러 1이다. 따라서 메인의 이전 대상은 없고, wd-yaml의 메인 몫은 3자 대조(같음 확인)뿐이다.
  이 범위는 h11로 결정됐다: wd-yaml 메인 몫은 이전 0이고, 하네스가 YAML로 읽는 216개가 구·신 파서에서 같은 값인지 `yaml_compat compare3`로 대조만 한다. 서브 몫은 plan대로다.
  추적 입력은 11개(manifest_contract 픽스처 X05~X09·Y01·YT17~YT19의 타임스탬프, LM21의 `yes`·타임스탬프, LM09의 병합 키)가 퇴역(c60, 원장 행) 대상이다.

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
  브리지와 py*는 c60이 빌드에서 빼고 `delete.txt`에 적는다. runtime-support `#[test]` 85개 + `checks/pycall_cases.rs` 6개는 모두 py* 쪽이므로 c60 `test_delta.tsv` 대상이다.
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
  각 cell이 최초 온라인 fetch(메인노드, 별도 실행 기록)로 판을 정하고 cell별 추가안으로 사람 승인을 받는다(D9). regex 퍼사드는 이미 고정된 `regex-automata 0.4.18`·`regex-syntax 0.8.11`과
  맞는 판을 고른다.
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
- `pyyaml/mod.rs:1`은 PyYAML 6.0.3, `pyyaml/sites.rs:1`과 wiki-desk 오라클은 6.0.1이라고 적는다.
- `docs/benchmark/benchmark_*.yaml`(메인 92개, 그중 30개에 `e_search: no`, 3개에 `*_backend_mismatch: no`)은 YAML 로더가 아니라 flat 문자열 파서(`completion_gate.rs:499`)가 읽는다.
  YAML 1.1 bool 의존 입력이 아니다(`yaml_inputs.tsv`).
