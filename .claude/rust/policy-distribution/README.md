# policy-distribution

Rust port of `.claude/policies/runtime/verify_distribution.py` (fail-closed acceptance of the five-skill self-contained
distribution). Entry: `bash .claude/policies/runtime/verify_distribution.sh [--json-out PATH]`
(native binary `target/release/verify_distribution`, built by `terraforming_node/scripts/build_rust_packages.sh`; a
missing binary is exit 2 with build guidance, never a Python start).

Check names and order, the report (`json.dumps(indent=2, ensure_ascii=False)` key order), exit codes (0 PASS · 1 FAIL ·
2 usage) and usage/help/error text (argparse, default 80 columns) follow the original, with these differences: the
checks `verdict_clock_scan` (`verdict_clock_scan.sh check --repo .`) and `verdict_clock_scan_selftest` (`--self-test`),
both expecting 0, run right after `inline_python_scan`; the policy audit runs as
`policy_registry.sh verify --repo-root <repo>` and the report has no `policy_as_of`; usage and help have no
`--policy-as-of`. The run reads no clock.

## Documented changes (intentional)

Checks that started one of the policy `.py` programs now start their Rust launcher; the check `name`, expected exit
codes and result shape are unchanged, only the child argv differs (the report never recorded argv):

| check | was | now |
|---|---|---|
| `antipattern_scan_selftest` | `python3 .claude/policies/runtime/antipattern_scan.py --self-test` | `bash .claude/policies/runtime/antipattern_scan.sh --self-test` |
| `constitution_lint_selftest` | `python3 …/constitution_lint.py --self-test` | `bash …/constitution_lint.sh --self-test` |
| `runtime_regression_selftest` | `python3 [-I -S -O -B] …/runtime_selftest.py` | `bash …/runtime_selftest.sh` |
| `doc_naming_shim_selftest` | `python3 …/doc_naming.py --self-test` | `bash …/doc_naming.sh --self-test` |
| `policy_registry_verify` | `python3 <repo>/…/policy_registry.py verify --as-of D --repo-root <repo>` | `bash <repo>/…/policy_registry.sh verify --repo-root <repo>`; the presence probe (and its `error`) names the `.sh` |
| `production_claim_predicates` | `python3 <repo>/.claude/policies/predicates/claim_predicates.py` | `bash <repo>/.claude/policies/predicates/claim_predicates.sh` |
| `production_companion_predicates` | `python3 -m unittest discover -s <dir> -p '*_predicate.py'` | every `<dir>/*_predicate.sh` in name order, first failure's code; exit 5 (unittest "no tests") when there is none |

Every other bundle's program (hint.sh, render_sub_env.sh, manifest_contract.sh, … and all `.sh` self-tests) also runs through
its Rust launcher (`bash <dir>/<x>.sh`; was `python3 <x>.py`, plan_26111615 — the `.py` programs are deleted).

Residual divergences (no gate decision on the repository's inputs depends on them; each is pinned by a check or noted):
- shipped-python `assert` scan: a keyword lexer (comments, all string forms incl. PEP 701 f-strings), not `ast.parse`;
  violations in source order (ast.walk is breadth-first); only null-byte / unterminated-string syntax errors are seen.
- TOML: a 1.0 subset reader (no date-times); error wording/position match tomllib for the detected cases.
- child output is decoded lossily (the original raised UnicodeDecodeError on non-UTF-8 child output).
- an exception the original left uncaught (unreadable `.dockerignore` / terraforming SKILL.md / skeletons,
  unwritable `--json-out`) ends the run with exit 1 and a short stderr notice instead of a Python traceback.
- help text is wrapped for the default 80 columns regardless of `COLUMNS`.

## Library (for `policy-selftest`'s runtime_selftest port)

- `policy_distribution::run(name, argv, expected, cwd, env_overrides) -> PyValue` — `_run`.
- `policy_distribution::shipped_python_paths(root) -> Vec<PathBuf>` — `_shipped_python_paths`.
- `policy_distribution::policy_registry_check(policy_runner, repo, |name, argv, expected| …)` — `_policy_registry_check`
  through the given `_run` (`verify_with` passes the `run` of its hooks).
- `policy_distribution::verify_with(repo, &Hooks{run, gitless, flag_issued})` — `verify()` with its three
  side-effecting seams injectable.

## Checks

`cargo test` (`checks/pure_goldens.rs`, 10 tests) compares against the goldens in `checks/data/`. Most are captures of
the original (its capture script was retired with the `.py`): the full static report with stubbed children on the
frozen tree of commit `b1f3563e` and on a mutated copy that fails every static check (`mutations.json`; the report
goldens without the retired `policy_as_of` fields), the assert detector over every shipped `.py` + snippets, TOML cases,
shell arrays / ordered-between, and `_run` results (rc, CRLF, 1000-char tails, missing program/cwd, signal, env).
`cli.json` holds the launcher's help and usage errors, recorded from the Rust launcher. Without a golden, a wiring test
checks the argv and expected exit codes of `verdict_clock_scan` and `verdict_clock_scan_selftest` (right after
`inline_python_scan`) and of `policy_registry_verify`. The checks never run Python.
Parity stage `pd` (`.claude/rust/parity/cases/pd.json`) has no case left: every recorded command line printed the
retired `--policy-as-of` (h20).
