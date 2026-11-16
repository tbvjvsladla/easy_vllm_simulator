# hint-publisher Rust 이행 기록

plan_26111615 D1(하네스 py 전부 Rust 이식) 의 hint-publisher 묶음. 원본 `scripts/hint.py` · `scripts/hintlib/*.py` ·
`scripts/render_bench_section.py` 는 라이브 1회 증거 전까지 **대조용으로 보존**한다(삭제는 P6). 이 패키지는 Python 을
띄우지 않는다 — 소유자가 다른 기능은 그 소유자의 native 진입점으로만 부르고, 없으면 `HINT_OWNER_LAUNCHER_MISSING`
으로 fail-closed 한다(대체 구현 ✗ · Python 대체 ✗).

## 빌드 · 진입점

```bash
bash .claude/skills/hint-publisher/scripts/build_rust.sh            # release 빌드 + hint --self-test
bash .claude/skills/terraforming_node/scripts/rust_bootstrap.sh exec -- \
  cargo test --locked --manifest-path .claude/skills/hint-publisher/rust/Cargo.toml
bash .claude/skills/hint-publisher/scripts/hint.sh --help             # 단일 CLI (원본 hint.py)
bash .claude/skills/hint-publisher/scripts/render_bench_section.sh    # 원본 render_bench_section.py
```

실행파일 부재 = exit 2 + 빌드 안내(설치·컴파일·Python 대체를 하지 않는다). `target/` 은 노드 로컬 생성물이다.

## 골든 대조

- 골든은 **원본 Python 을 실행해** 캡처하고 커밋한다: `.claude/rust/parity/cases/hp_<module>.json` →
  `.claude/rust/parity/data/HP-<module>-*/`. 캡처는 `parity capture --stage hp_<module> --repo "$PWD"` 이며 원본은
  `orig_rev` 의 git archive 에서 돈다(작업트리의 Rust 빌드물이 섞이지 않는다).
- 대조는 `parity compare --stage hp_<module> --repo "$PWD"` — Rust 진입점(`scripts/hint_api.sh` · `hint.sh` ·
  `render_bench_section.sh`)만 실행하고 strace 로 Python exec 0 을 함께 판정한다. **Rust 검사는 .py 를 실행하지 않는다.**
- 라이브러리 함수는 JSON-lines 다리(`hint_api` · 원본 쪽 = `.claude/rust/parity/cases/shims/hint_api.py`)로 잰다.
  요청/응답/값 인코딩 계약은 `src/api/mod.rs` 머리말이 정본이다.

## 이식 규약 (모듈 담당자 공통)

- 모듈 = 파일 하나(`src/<module>.rs`, 크면 `src/<module>/…`), 함수 이름 = Python 이름(snake_case 그대로 · 선행 `_` 는
  `p_` 없이 그대로 private fn). 상수는 `pub const`/`pub static` 또는 `fn NAME() -> &'static …`.
- 값: Python `dict`/`list` 는 `value::J`(순서 보존 `Map`). dataclass 는 같은 이름의 `pub struct`(필드 순서 = 선언 순서).
- 실패: `core::HintError`/`HR<T>`. `core.fail(...)` → `return core::fail(code, msg, remedy)`. 원본이 번역하지 않고
  흘려보내는 내장 예외(`ValueError` 등)는 `HintError::py("ValueError", str(e))`.
- 정규식: `pyre::Re`(Python 철자 · `re_static!`). `$`(re.M 없음)·`\Z` 는 자동 번역된다.
- 출력: `println!`/`eprintln!` 금지 — `io::out/outln/err/errln`(다리가 출력을 캡처한다).
- JSON 쓰기: `core::dumps`(sort_keys · indent 2 · ensure_ascii=False) · 다른 모양은 `value::dumps_with`.
- 시각은 주입만. 벽시계를 읽지 않는다.
- 소유자 경계: 다른 스킬·정책의 Python 모듈을 in-process 로 부르던 자리(`core.load_owner_module`)는
  ① 소유자의 Rust 라이브러리(경로 의존 · 읽기 전용) ② 소유자의 native 실행파일/launcher(`core::run_owner`)
  ③ 둘 다 없으면 **같은 규칙의 미러 + 그 사실의 기록**(아래 표) 중 하나로 옮긴다. 미러는 그 소유자가 Rust 를 갖게 되면
  ①/②로 바꾼다.

## 모듈 상태

| 원본 | Rust | 상태 |
|---|---|---|
| hintlib/core.py | src/core.rs | ported · 골든 HP-core-api |
| hintlib/naming.py | src/naming.rs | pending |
| hintlib/pii.py | src/pii.rs | pending |
| hintlib/lineage.py | src/lineage.rs | pending |
| hintlib/catalog.py | src/catalog.rs | pending |
| hintlib/branch.py | src/branch.rs | pending |
| hintlib/tag.py | src/tag.rs | pending |
| hintlib/template.py | src/template.rs | pending |
| hintlib/artifacts.py | src/artifacts.rs | pending |
| hintlib/evidence.py | src/evidence.rs | pending |
| hintlib/selftest_image_git_release.py | (checks) | pending |
| render_bench_section.py | src/render_bench_section.rs | pending |
| hint.py | src/cli.rs | pending |

## 의도된 차이

- `core.REL_HINT_CLI` = `scripts/hint.sh`(원본 `scripts/hint.py`) — 안내문이 사람을 Python 진입점으로 보내지 않는다.
- `core.load_owner_module` 없음(위 소유자 경계). `core.run_python` → `core::run_owner`(소유자의 `<stem>.sh` launcher).
