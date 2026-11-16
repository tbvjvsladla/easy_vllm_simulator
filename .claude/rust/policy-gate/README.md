# policy-gate

Rust ports of `.claude/policies/runtime/completion_gate.py` and `doc_naming.py` (lib `policy_gate`, bins
`completion_gate` / `doc_naming`, launchers `.claude/policies/runtime/completion_gate.sh` / `doc_naming.sh`).
Build: `bash .claude/skills/terraforming_node/scripts/build_rust_packages.sh`.

## Library API for the other policy ports

`policy_gate::completion_gate` (Python name -> Rust):

| Python | Rust |
|---|---|
| `parse_flat_certificate(text)` | `parse_flat_certificate(&str) -> (Fields, bool)` (`Fields::get(key)`) |
| `certificate_run_key(fields)` | `certificate_run_key(&Fields) -> Option<Vec<String>>` |
| `CERTIFICATE_RUN_KEY_FIELDS` | `CERTIFICATE_RUN_KEY_FIELDS: [&str; 7]` |
| `certificate_grade` | `certificate_grade(&Fields) -> Option<&str>` |
| `certificate_requirement_authority` | `certificate_requirement_authority(&PyValue) -> Option<String>` |
| `_certificate_strong_identity_matches` | `certificate_strong_identity_matches(&Fields, &PyValue)` |
| `_find_repo_root` | `find_repo_root(&Path) -> Option<PathBuf>` |
| `_is_absolute_path_string` | `is_absolute_path_string(&str)` |
| `_lexical_components` / `lexical_components` | `lexical_components(manifest_dir, repo_root, rel) -> Option<Vec<String>>` (None = repo_escape) |
| `resolve_and_stat_evidence` | `resolve_and_stat_evidence(&Fd, repo_root, manifest_dir, rel, expect_dir, capture) -> Resolved` |
| `_walk_nofollow` | `walk_nofollow(&Fd, &[String]) -> Result<Fd, Walk>` |
| `check_required_local_links` | `check_required_local_links(&Fd, repo_root, base_dir, &[u8])` |
| `required_evidence_for` | `required_evidence_for(task_class, Option<&PyValue>, Option<&PyValue>, Option<&str>)` |
| `validate_against_schema` | re-export of `agent_runtime_support::schema::validate_against_schema` |
| `WORK_SCHEMA` / `SIDE_EFFECT_SCHEMA` / `COMPLETION_SCHEMA` | `Schemas::load(&Env)` or `load_schema(schema_dir, file)` |
| `_emit_with_schema` | `emit_with_schema(PyValue, exit_code, &schema) -> Result<Exit, String>` |
| `bench_report_verdict_table` | `bench_report_verdict_table(Option<&str>)` |
| `_manifest_rubric_contract` | `manifest_rubric_contract(&PyValue) -> ManifestRubric` |
| `_is_valid_utc_timestamp` | `is_valid_utc_timestamp(Option<&PyValue>)` |
| constants | `LITE_CERT_VERDICT`, `OBSERVATION_ONLY_VERDICT`, `OBSERVATION_ONLY_JUDGE_VERDICT`, `PII_MACHINE_RAW_EXEMPTION_REASON`, `RUBRIC_AUTHORITIES`, `ALLOWED_ACTIONS`, `MANIFEST_RUBRIC_SOURCES`, `STRONG_IDENTITY_FIELDS`, `CERTIFICATE_FIELD_MAP` |
| `cmd_verify` / `cmd_authorize` | `cmd_verify(&Schemas, &Env, manifest, repo_root)` / `cmd_authorize(...)` |

`policy_gate::doc_naming::DocNaming::load(repo)` then methods `validate_topic`, `is_dated_doc_basename`,
`dated_doc_basename`, `simlog_dirname`, `report_basename`, `kst_tokens`, `gpu_key`, `measurement_ts`, `scan_bench_dir`,
`bench_filename`; fields `dated_doc_types`, `topic_pattern` / `dated_doc_pattern` / `report_pattern` (Python spelling);
errors `NamingError::{CollisionExhausted, SourceUnreadable, Value, Other}`.

## Goldens

`checks/data/` holds outputs captured from the original Python by `checks/capture_goldens.sh` (re-capture needs the
original `.py`); `cargo test` compares the Rust binaries against them and never executes Python.

## Known divergences (documented, not silent)

* `authorize` runs the pre-side-effect tripwires through `runtime_selftest.sh` (the Rust launcher) beside this file
  instead of `runtime_selftest.py`; the SKIPPED line names `runtime_selftest.sh`.
* Help texts are the 80-column argparse rendering (Python re-wraps for a different `COLUMNS`).
* `casefold()` of Markdown reference labels covers the lowercase mapping plus the common full foldings (ß, ς, ſ, µ,
  Latin ligatures), not the whole Unicode CaseFolding table.
