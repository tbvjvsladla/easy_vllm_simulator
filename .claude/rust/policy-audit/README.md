# policy-audit

Rust port of `.claude/policies/runtime/policy_registry.py` (the registry validator/auditor) and
`.claude/policies/runtime/harness_verify.py` (the three-stage completion gate). Unpublished (`publish = false`).

| Original | Binary | Launcher |
|---|---|---|
| `policy_registry.py verify / check-removal / --self-test` | `policy_registry` | `.claude/policies/runtime/policy_registry.sh` |
| `harness_verify.py` | `harness_verify` | `.claude/policies/runtime/harness_verify.sh` |

The launchers start the node-built release binary directly (`exec -a <same dir>/<name>.sh`: the argparse `prog` text names the
launcher and the repository root is derived from the same directory); a missing binary is exit 2 with the build command
(`bash .claude/skills/terraforming_node/scripts/build_rust_packages.sh`). There is no Python fallback. `POLICY_REGISTRY_BIN` /
`HARNESS_VERIFY_BIN` override the binary path (used by the checks).

## Library API (for the other policies packages)

`policy_audit::` re-exports, with the original's names and messages:

- `Violation` (`reason_code`, `message`, `policy_id`, `clause_id`, `path`; `to_dict()`)
- `scan_policy_citations(text) -> Vec<(line, token)>`, `citation_violations(text, registry_doc, source_label)`
- `evaluate_lifecycle(doc, repo_root, Option<claim_bindings>)`,
  `claim_binding_violations(policies, claim_bindings)`, `evidence_structural_violations`, `clause_executable_evidence_violations`
- `arch_variant_contract_violations(repo_root)`, `git_available(repo_root)` (`_git_available`), `resolve_evidence_path`
- `schema_meta_violations`, `schema_violations`, `safe_load_json_object`, `compute_contract_tree_sha256`,
  `check_removal_eligibility`, `check_removal_full`, `extract_assertion_ids`
- `self_test(repo_root)` (`policy_registry._self_test`)
- No date rule and no clock (h20, 2026-11-19): the review-cycle state machine, the 90-day rules, the date-chronology rules and
  `--as-of` are gone; the registry fields they read (`added_at`, `last_reproduced`, `review`) left the schema (v3).
- `registry::policies_of(doc)` = `[p for p in doc["policies"] if isinstance(p, dict)]`

## Behaviour notes

- **Assertion ids for `.py` evidence**: the original used CPython `ast` (top-level `def`, top-level `Assign`/`AnnAssign`
  name targets, `Class.test*` of any `TestCase` subclass). `pyids.rs` reads the same facts with a small Python lexer; on every
  `.py` file and evidence path of commit `b1f3563e` the sets are identical (`checks/ids_golden.rs`). An unterminated
  string/bracket is `None` like a `SyntaxError`; other syntax errors are not detected. `.rs`, `.ts`, `.sh`, `.yaml/.yml`, `.md`
  and `.json` evidence use the original's regex/lexical rules unchanged (the `.rs` kind already existed in the original), so
  registry entries can be re-pointed at Rust files without any change here.
- `clause_executable_evidence_violations` reports clauses in sorted clause-id order; the original iterated a Python `set`
  (hash-randomized order), so its order for two or more such clauses in one policy was not deterministic.
- The arch-variant selector check derives `image_identity` with the upstream owner's Rust library
  (`upstream_runtime::slave_forward::candidates`) instead of importing `slave_forward.py`; it still requires the owner files
  to exist (the original's "cannot judge" gate): the multi compose template and `arch::SLAVE_FORWARD_REL`, which now names the
  upstream Rust module `.claude/skills/upstream-version-watch/rust/runtime/src/slave_forward.rs` (the deleted `slave_forward.py` became it plus `slave_forward.sh`).
- The contract-tree digest hashes the bytes of the validator source, now `.claude/rust/policy-audit/src/registry.rs`
  (`CONTRACT_SCRIPT_REL`; the original hashed its own `policy_registry.py`). No policy carries a removal artifact, so no stored digest depends on the old input.
- `--self-test`'s OK fixture cites `.claude/rust/runtime-support/src/schema.rs :: validate_against_schema` (tracked Rust)
  instead of `policy_registry.py :: evaluate_lifecycle`, so it survives the deletion of the Python original.
- `harness_verify` runs its stages through the Rust launchers (`policy_registry.sh verify`, `runtime_selftest.sh`,
  `predicates/claim_predicates.sh`, then `predicates/companion_predicate.sh` and `predicates/runtime_patch_predicate.sh` in
  place of `unittest discover -p '*_predicate.py'`). A missing stage launcher fails that stage closed. stdout keeps CPython's
  buffering (children first, harness lines at exit, when stdout is not a terminal).

## Checks

`cargo test --release --offline --locked` runs:

- `ids_golden` — assertion-id sets vs. `checks/data/assertion_ids.json` (frozen — capture tool retired).
- `cli_golden` — the CLI cases in `checks/data/cli/<id>/` (fixture `case.sh` + captured `stdout`/`stderr`/`exit_code`;
  frozen — capture tool retired; re-run here through the launchers by `checks/run_case.sh`).
  Fixtures are git repos built from commit `b1f3563e`'s blobs or literal text (helpers in `checks/fixture_lib.sh`).
- `citations_golden` — `scan_policy_citations` / `citation_violations` vs. `checks/data/citations.json`
  (frozen — capture tool retired).
- `harness_checks` — stage order / exit-code mapping with fake stage launchers.

The goldens are frozen: their capture tools were retired with the Python originals (plan_26111717 R1, 28556136), so they are not
re-captured; the checks themselves never run Python.
