#!/bin/bash
# Golden scenario driver for evidence_publisher (Python original or Rust port — the program is an argument).
#   bash scenarios.sh OUTDIR ROOT PROGRAM [ARGS...]
# ROOT is wiped and used as a hermetic --repo-root. For every step NAME the driver writes OUTDIR/NAME.{out,err,rc,tree}
# (stdout, stderr, exit code, and the full docs/ tree after the step), with the literal ROOT path replaced by <ROOT>.
# The same steps run for both programs; checks/goldens.rs compares the Rust run with the committed Python capture.
# No Python here: the driver is bash, the program under test is whatever the caller names.
set -u
OUT="$1"; ROOT="$2"; shift 2
PUB=("$@")
rm -rf -- "$ROOT" "$OUT"
mkdir -p -- "$ROOT" "$OUT"
cd "$ROOT" || exit 9

norm() { local s; s="$(cat; printf x)"; s="${s%x}"; printf '%s' "${s//"$ROOT"/<ROOT>}"; }
tree() {
  ( cd "$ROOT" || exit 9
    find docs -mindepth 1 2>/dev/null | LC_ALL=C sort | while IFS= read -r p; do
      if [ -L "$p" ]; then printf '== %s -> %s\n' "$p" "$(readlink "$p")"
      elif [ -d "$p" ]; then printf '== %s/\n' "$p"
      else printf '== %s\n' "$p"; cat -- "$p"; printf '\n'; fi
    done )
}
st() {
  local name="$1"; shift
  "${PUB[@]}" "$@" >"$OUT/$name.out.raw" 2>"$OUT/$name.err.raw" </dev/null
  echo "$?" >"$OUT/$name.rc"
  norm <"$OUT/$name.out.raw" >"$OUT/$name.out"; norm <"$OUT/$name.err.raw" >"$OUT/$name.err"
  rm -f -- "$OUT/$name.out.raw" "$OUT/$name.err.raw"
  tree | norm >"$OUT/$name.tree"
}
R=(--repo-root "$ROOT")
w() { mkdir -p -- "$(dirname -- "$ROOT/$1")"; printf '%s' "$2" >"$ROOT/$1"; }

for d in plan devlog testlog simlog benchmark report _evidence/inputs; do mkdir -p "$ROOT/docs/$d"; done
w identity.json '{"model": "self-test-model", "gpu": "GB10", "vllm": "0.0.0", "quant": null, "topology": "single", "tp": 1}'
w identity_bad.json '{"model": "m", "gpu": "GB10"}'
w array.json '[1, 2]'
w invalid.json '{"model": '
printf '\xff\xfe' >"$ROOT/badutf8.json"
w cond_bad.json '{"actual_trial": "yes"}'
w cond_audit.json '{"report_requested": true}'
w cond_trial.json '{"actual_trial": true}'

# ── CLI surface ──────────────────────────────────────────────────────────────────────────────────────────────
st cli_none
st cli_bogus bogus
st cli_help --help
for c in init append-raw set-narrative record-capacity-rejection publish-benchmark publish-lite-report set-promotion-target finalize; do
  st "cli_help_$c" "$c" -h
done
st cli_init_missing init
st cli_init_choice init --topic t --task-class nope --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json
st cli_unrecognized init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json --extra
st cli_expected_one init --topic
st cli_ambiguous init --re x --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json
st cli_selftest_and_cmd --self-test finalize --topic x --bogus
st cli_append_kind append-raw "${R[@]}" --topic t --kind nope --src x --recorded-utc 2026-01-01T00:00:00Z
st cli_double_dash init -- --topic

# ── init input errors (no writes) ───────────────────────────────────────────────────────────────────────────
st init_topic_escape init "${R[@]}" --topic ../x --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json
st init_topic_space init "${R[@]}" --topic "a b" --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json
st init_topic_default_root init --topic ../x --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json
st init_bad_utc init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-99-99T99:99:99Z --identity-json identity.json
st init_bad_utc_shape init "${R[@]}" --topic t --task-class minor_patch --generated-utc "2026-01-01 00:00" --identity-json identity.json
st init_identity_missing init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json nope.json
st init_identity_invalid init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json invalid.json
st init_identity_utf8 init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json badutf8.json
st init_identity_dir init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json docs
st init_identity_array init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json array.json
st init_identity_schema init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json identity_bad.json
st init_conditions_array init "${R[@]}" --topic t --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json --conditions-json array.json
st init_conditions_schema init "${R[@]}" --topic t --task-class harness_change --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json --conditions-json cond_bad.json
st init_report_slug_required init "${R[@]}" --topic audit --task-class read_only_audit --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json --conditions-json cond_audit.json
st init_report_slug_invalid init "${R[@]}" --topic audit --task-class read_only_audit --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json --conditions-json cond_audit.json --report-slug rtxpro6000-benchmark-explorer
st init_audit_report init "${R[@]}" --topic audit --task-class read_only_audit --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json --conditions-json cond_audit.json --report-slug perf_26010109_감사_보고
st init_audit_report_again init "${R[@]}" --topic audit --task-class read_only_audit --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json --conditions-json cond_audit.json --report-slug perf_26010109_감사_보고
st init_trial init "${R[@]}" --topic trial --task-class harness_change --generated-utc 2026-01-01T00:30:29Z --identity-json identity.json --conditions-json cond_trial.json
st init_trial_rebind init "${R[@]}" --topic trial --task-class harness_change --generated-utc 2026-01-01T00:30:30Z --identity-json identity.json
rm -f "$ROOT"/docs/plan/plan_*_trial.md
st init_trial_restore init "${R[@]}" --topic trial --task-class harness_change --generated-utc 2026-01-01T00:30:29Z --identity-json identity.json --conditions-json cond_trial.json

# ── minor_patch roundtrip (append-raw sidecar ledger + finalize) ───────────────────────────────────────────
st append_uninit append-raw "${R[@]}" --topic selftest --kind verification --src verify.txt --recorded-utc 2026-01-01T00:01:00Z
st mp_init init "${R[@]}" --topic selftest --task-class minor_patch --generated-utc 2026-01-01T00:00:00Z --identity-json identity.json
w verify.txt $'py_compile OK\n'
w commit.txt $'deadbeef\n'
w empty.txt ''
ln -s verify.txt "$ROOT/link.txt"
st mp_append_verif append-raw "${R[@]}" --topic selftest --kind verification --src verify.txt --recorded-utc 2026-01-01T00:01:00Z
st mp_append_commit append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:01:00Z
st mp_append_commit2 append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:02:00Z
st mp_append_bench append-raw "${R[@]}" --topic selftest --kind bench_report --src commit.txt --recorded-utc 2026-01-01T00:02:00Z
st mp_append_abs append-raw "${R[@]}" --topic selftest --kind commit --src /etc/hostname --recorded-utc 2026-01-01T00:02:00Z
st mp_append_escape append-raw "${R[@]}" --topic selftest --kind commit --src ../x.txt --recorded-utc 2026-01-01T00:02:00Z
st mp_append_missing append-raw "${R[@]}" --topic selftest --kind commit --src missing.txt --recorded-utc 2026-01-01T00:02:00Z
st mp_append_empty append-raw "${R[@]}" --topic selftest --kind commit --src empty.txt --recorded-utc 2026-01-01T00:02:00Z
st mp_append_symlink append-raw "${R[@]}" --topic selftest --kind commit --src link.txt --recorded-utc 2026-01-01T00:02:00Z
st mp_append_utf8 append-raw "${R[@]}" --topic selftest --kind commit --src badutf8.json --recorded-utc 2026-01-01T00:02:00Z
st mp_append_dir append-raw "${R[@]}" --topic selftest --kind commit --src docs --recorded-utc 2026-01-01T00:02:00Z
st mp_append_simlog_noscaffold append-raw "${R[@]}" --topic selftest --kind simlog --src commit.txt --recorded-utc 2026-01-01T00:02:00Z
st mp_append_bad_utc append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc nope
w pii.json '{"passed": true, "scanned_paths": ["../../verify.txt", "../../commit.txt"]}'
w runtime.json '{"health_ok": true, "functional_smoke_passed": true, "identity": {"model": "self-test-model", "gpu": "GB10", "vllm": "0.0.0", "quant": null, "topology": "single", "tp": 1}, "containers": [{"name": "svc", "restart_count": 0, "oom_killed": false}]}'
st mp_finalize finalize "${R[@]}" --topic selftest --pii-scan-json pii.json --runtime-json runtime.json
st mp_finalize_nopii finalize "${R[@]}" --topic selftest
st mp_finalize_pii_missing finalize "${R[@]}" --topic selftest --pii-scan-json none.json
st mp_finalize_runtime_invalid finalize "${R[@]}" --topic selftest --runtime-json invalid.json
st mp_finalize_runtime_array finalize "${R[@]}" --topic selftest --runtime-json array.json
st mp_capacity_wrong record-capacity-rejection "${R[@]}" --topic selftest --gate-evidence-src commit.txt --reason r --recorded-utc 2026-01-01T00:02:00Z
st mp_bench_wrong publish-benchmark "${R[@]}" --topic selftest --verdict PASS --generated-utc 2026-01-01T00:02:00Z --bench-report-src x
st mp_lite_wrong publish-lite-report "${R[@]}" --topic selftest --generated-utc 2026-01-01T00:02:00Z --bench-report-src x
st mp_narr_kind set-narrative "${R[@]}" --topic selftest --kind plan --narrative-file commit.txt --author a --generated-utc 2026-01-01T00:02:00Z

# ── ledger tampering ────────────────────────────────────────────────────────────────────────────────────────
cp "$ROOT/docs/_evidence/selftest.commit.raw.jsonl" "$ROOT/ledger.bak"
printf '{"seq": 1}' >"$ROOT/docs/_evidence/selftest.commit.raw.jsonl"
st ledger_noterm append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:03:00Z
printf '{"seq": 1}\n{"seq": 3}\n' >"$ROOT/docs/_evidence/selftest.commit.raw.jsonl"
st ledger_gap append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:03:00Z
printf '{"seq": 1}\n\n' >"$ROOT/docs/_evidence/selftest.commit.raw.jsonl"
st ledger_blank append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:03:00Z
printf '[1]\n' >"$ROOT/docs/_evidence/selftest.commit.raw.jsonl"
st ledger_array append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:03:00Z
printf '{"seq": true}\n' >"$ROOT/docs/_evidence/selftest.commit.raw.jsonl"
st ledger_bool append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:03:00Z
printf '{"seq": 1\n' >"$ROOT/docs/_evidence/selftest.commit.raw.jsonl"
st ledger_json append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:03:00Z
printf '\xff\n' >"$ROOT/docs/_evidence/selftest.commit.raw.jsonl"
st ledger_utf8 append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:03:00Z
rm -f "$ROOT/docs/_evidence/selftest.commit.raw.jsonl"; ln -s ../../ledger.bak "$ROOT/docs/_evidence/selftest.commit.raw.jsonl"
st ledger_symlink append-raw "${R[@]}" --topic selftest --kind commit --src commit.txt --recorded-utc 2026-01-01T00:03:00Z
rm -f "$ROOT/docs/_evidence/selftest.commit.raw.jsonl"; cp "$ROOT/ledger.bak" "$ROOT/docs/_evidence/selftest.commit.raw.jsonl"

# ── record tampering ────────────────────────────────────────────────────────────────────────────────────────
cp "$ROOT/docs/_evidence/selftest.json" "$ROOT/record.bak"
rec() { printf '%s' "$1" >"$ROOT/docs/_evidence/selftest.json"; }
rec '{"x": '; st rec_invalid_json finalize "${R[@]}" --topic selftest
rec '[]'; st rec_array finalize "${R[@]}" --topic selftest
rec '{}'; st rec_empty finalize "${R[@]}" --topic selftest
printf '\xff' >"$ROOT/docs/_evidence/selftest.json"; st rec_utf8 finalize "${R[@]}" --topic selftest
sed 's/"publication_id": "selftest"/"publication_id": "other"/' "$ROOT/record.bak" >"$ROOT/docs/_evidence/selftest.json"; st rec_topic_mismatch finalize "${R[@]}" --topic selftest
sed 's/"schema_version": 1,/"schema_version": 1, "zzz": 1,/' "$ROOT/record.bak" >"$ROOT/docs/_evidence/selftest.json"; st rec_unknown finalize "${R[@]}" --topic selftest
sed 's/"generated_utc": "2026-01-01T00:00:00Z"/"generated_utc": "2026-01-01"/' "$ROOT/record.bak" >"$ROOT/docs/_evidence/selftest.json"; st rec_bad_utc finalize "${R[@]}" --topic selftest
sed 's/"task_class": "minor_patch"/"task_class": "harness_change"/' "$ROOT/record.bak" >"$ROOT/docs/_evidence/selftest.json"; st rec_required_mismatch finalize "${R[@]}" --topic selftest
sed 's/"verification": "verify.txt"/"verification": 5/' "$ROOT/record.bak" >"$ROOT/docs/_evidence/selftest.json"; st rec_scaffold_value finalize "${R[@]}" --topic selftest
sed 's|"commit": "docs/_evidence/selftest.commit.raw.jsonl"|"commit": "docs/_evidence/other.jsonl"|' "$ROOT/record.bak" >"$ROOT/docs/_evidence/selftest.json"; st rec_raw_mismatch finalize "${R[@]}" --topic selftest
sed 's/"capacity_rejection_required": false/"capacity_rejection_required": 0/' "$ROOT/record.bak" >"$ROOT/docs/_evidence/selftest.json"; st rec_capflag finalize "${R[@]}" --topic selftest
sed 's/"quant": null/"quant": 5/' "$ROOT/record.bak" >"$ROOT/docs/_evidence/selftest.json"; st rec_schema finalize "${R[@]}" --topic selftest
rm -f "$ROOT/docs/_evidence/selftest.json"; ln -s ../../record.bak "$ROOT/docs/_evidence/selftest.json"; st rec_symlink finalize "${R[@]}" --topic selftest
rm -f "$ROOT/docs/_evidence/selftest.json"; mkdir "$ROOT/docs/_evidence/selftest.json"; st rec_dir finalize "${R[@]}" --topic selftest
rmdir "$ROOT/docs/_evidence/selftest.json"; cp "$ROOT/record.bak" "$ROOT/docs/_evidence/selftest.json"

# ── capacity_rejection ──────────────────────────────────────────────────────────────────────────────────────
st cap_init init "${R[@]}" --topic cap --task-class capacity_rejection --generated-utc 2026-01-01T01:00:00Z --identity-json identity.json
w gate.txt $'KV budget exceeded\n'
st cap_record record-capacity-rejection "${R[@]}" --topic cap --gate-evidence-src gate.txt --reason "kv budget" --recorded-utc 2026-01-01T01:01:00Z
st cap_record_missing record-capacity-rejection "${R[@]}" --topic cap --gate-evidence-src nope.txt --reason r --recorded-utc 2026-01-01T01:01:00Z
w docs/_evidence/inputs/cap_plan.md $'# plan\ncapacity plan.\n'
st cap_narr set-narrative "${R[@]}" --topic cap --kind plan --narrative-file docs/_evidence/inputs/cap_plan.md --author tester --generated-utc 2026-01-01T01:02:00Z
st cap_finalize finalize "${R[@]}" --topic cap --pii-scan-json pii.json
w capj.json '{"rejected": true, "gate_evidence": "cap_capacity_gate.txt", "reason": "override", "recorded_utc": "2026-01-01T01:05:00Z"}'
st cap_finalize_json finalize "${R[@]}" --topic cap --capacity-rejection-json capj.json

# ── narrative guards & injection ────────────────────────────────────────────────────────────────────────────
st nar_init init "${R[@]}" --topic nar --task-class harness_change --generated-utc 2026-01-01T07:00:00Z --identity-json identity.json
w docs/_evidence/inputs/n.md $'서사 본문\n\n'
w docs/_evidence/inputs/marker.md $'x <!-- NARRATIVE:BEGIN --> y\n'
st nar_author_ctrl set-narrative "${R[@]}" --topic nar --kind plan --narrative-file docs/_evidence/inputs/n.md --author $'a\tb' --generated-utc 2026-01-01T07:01:00Z
st nar_author_comment set-narrative "${R[@]}" --topic nar --kind plan --narrative-file docs/_evidence/inputs/n.md --author 'a-->b' --generated-utc 2026-01-01T07:01:00Z
st nar_author_long set-narrative "${R[@]}" --topic nar --kind plan --narrative-file docs/_evidence/inputs/n.md --author "$(printf 'x%.0s' {1..201})" --generated-utc 2026-01-01T07:01:00Z
st nar_author_empty set-narrative "${R[@]}" --topic nar --kind plan --narrative-file docs/_evidence/inputs/n.md --author '' --generated-utc 2026-01-01T07:01:00Z
st nar_source_comment set-narrative "${R[@]}" --topic nar --kind plan --narrative-file 'docs/<!--x.md' --author a --generated-utc 2026-01-01T07:01:00Z
st nar_marker set-narrative "${R[@]}" --topic nar --kind plan --narrative-file docs/_evidence/inputs/marker.md --author a --generated-utc 2026-01-01T07:01:00Z
st nar_report_kind set-narrative "${R[@]}" --topic nar --kind report --narrative-file docs/_evidence/inputs/n.md --author a --generated-utc 2026-01-01T07:01:00Z
st nar_inject set-narrative "${R[@]}" --topic nar --kind plan --narrative-file docs/_evidence/inputs/n.md --author "작성자 ✓" --generated-utc 2026-01-01T07:01:00Z
st nar_inject_again set-narrative "${R[@]}" --topic nar --kind plan --narrative-file docs/_evidence/inputs/n.md --author a --generated-utc 2026-01-01T07:02:00Z
DEV="$(cd "$ROOT" && ls docs/devlog/devlog_*_nar.md)"
printf '# broken\n<!-- NARRATIVE:END -->\n<!-- NARRATIVE:BEGIN -->\n' >"$ROOT/$DEV"
st nar_markers_reversed set-narrative "${R[@]}" --topic nar --kind devlog --narrative-file docs/_evidence/inputs/n.md --author a --generated-utc 2026-01-01T07:03:00Z
printf '# broken\n' >"$ROOT/$DEV"
st nar_markers_missing set-narrative "${R[@]}" --topic nar --kind devlog --narrative-file docs/_evidence/inputs/n.md --author a --generated-utc 2026-01-01T07:03:00Z
rm -f "$ROOT/$DEV"
st nar_target_missing set-narrative "${R[@]}" --topic nar --kind devlog --narrative-file docs/_evidence/inputs/n.md --author a --generated-utc 2026-01-01T07:03:00Z
w docs/testlog/testlog_26010116_nar_bound.md $'# testlog\n진짜 본문.\n'
st nar_bind set-narrative "${R[@]}" --topic nar --kind testlog --narrative-file docs/testlog/testlog_26010116_nar_bound.md --author a --generated-utc 2026-01-01T07:04:00Z
st nar_bind_again set-narrative "${R[@]}" --topic nar --kind testlog --narrative-file docs/testlog/testlog_26010116_nar_bound.md --author b --generated-utc 2026-01-01T07:05:00Z
st nar_reinit init "${R[@]}" --topic nar --task-class harness_change --generated-utc 2026-01-01T07:00:00Z --identity-json identity.json

# ── full_benchmark binding (mirrors the built-in self-test, with outputs captured) ──────────────────────────
CERT=$'schema_version: 1\nrecord_type: benchmark_certificate\nverdict: PASS\nmodel: self-test-model\ngpu_model: GB10\nvllm_version: 0.0.0\nquantization: N/A\ntopology: single\ntensor_parallel_size: 1\nbenchmark_mode: full\ndecode_tps_conc1: 12.3\nrubric_authority: weak\nprimary_source: expected_achievable(roofline×MBU)\nprimary_tps: 10.0\nfloor_tps: 8.5\ntolerance: 0.15\nratio_M_over_primary: 1.23\nmeasured_utc: "2026-01-01T00:00:00Z"\n'
STEM=26010109_self-test-model_GB10_0.0.0
CERT_REL=docs/benchmark/benchmark_$STEM.yaml
REP_REL=docs/benchmark/bench_report_$STEM.md
w "$CERT_REL" "$CERT"
w "$REP_REL" $'# report\n생성일 2026-01-01T00:00:00Z.\n'
st b_init init "${R[@]}" --topic bench --task-class full_benchmark --generated-utc 2026-01-01T02:00:00Z --identity-json identity.json --benchmark-mode full --benchmark-verdict PASS
pb() { local name="$1" verdict="$2"; shift 2; st "$name" publish-benchmark "${R[@]}" --topic bench --verdict "$verdict" --generated-utc 2026-01-01T02:05:00Z "$@"; }
pb b_pass PASS --bench-report-src "$REP_REL" --certificate-src "$CERT_REL"
pb b_pass_again PASS --bench-report-src "$REP_REL" --certificate-src "$CERT_REL"
w docs/_evidence/inputs/x.yaml "$CERT"
pb b_noncanon PASS --bench-report-src "$REP_REL" --certificate-src docs/_evidence/inputs/x.yaml
pb b_noncanon_abs PASS --bench-report-src "/abs/$REP_REL"
pb b_noncanon_name PASS --bench-report-src docs/benchmark/report.md
DUP=docs/benchmark/benchmark_26010109_00_00_self-test-model_GB10_0.0.0.yaml
w "$DUP" "$CERT"
pb b_dup_identical PASS --bench-report-src "$REP_REL" --certificate-src "$CERT_REL"
w "$DUP" "$CERT# differs"$'\n'
pb b_dup_different PASS --bench-report-src "$REP_REL" --certificate-src "$CERT_REL"
rm -f "$ROOT/$DUP"
w docs/benchmark/benchmark_26010109_99_99_unparseable.yaml $'a:\n  nested: 1\n'
pb b_pass_with_unparseable_sibling PASS --bench-report-src "$REP_REL" --certificate-src "$CERT_REL"
rm -f "$ROOT/docs/benchmark/benchmark_26010109_99_99_unparseable.yaml"
OTHER_REP=docs/benchmark/bench_report_26010110_self-test-model_GB10_0.0.0.md
w "$OTHER_REP" $'# r\n생성일 2026-01-01T01:00:00Z.\n'
pb b_stem PASS --bench-report-src "$OTHER_REP" --certificate-src "$CERT_REL"
rm -f "$ROOT/$OTHER_REP"
pb b_cert_without_pass REFUTE --bench-report-src "$REP_REL" --certificate-src "$CERT_REL"
for mut in verdict:REFUTE mode:lite model:other; do
  k="${mut%%:*}"; v="${mut#*:}"; f=docs/benchmark/benchmark_26010120_self-test-model_GB10_0.0.0.yaml
  case "$k" in
    verdict) w "$f" "${CERT/verdict: PASS/verdict: $v}" ;;
    mode) w "$f" "${CERT/benchmark_mode: full/benchmark_mode: $v}" ;;
    model) w "$f" "${CERT/model: self-test-model/model: $v}" ;;
  esac
  w docs/benchmark/bench_report_26010120_self-test-model_GB10_0.0.0.md $'# r\n'
  pb "b_cert_$k" PASS --bench-report-src docs/benchmark/bench_report_26010120_self-test-model_GB10_0.0.0.md --certificate-src "$f"
done
w docs/benchmark/benchmark_26010120_self-test-model_GB10_0.0.0.yaml $'a:\n  nested: 1\n'
pb b_cert_unparseable PASS --bench-report-src docs/benchmark/bench_report_26010120_self-test-model_GB10_0.0.0.md --certificate-src docs/benchmark/benchmark_26010120_self-test-model_GB10_0.0.0.yaml
w docs/benchmark/benchmark_26010120_self-test-model_GB10_0.0.0.yaml "${CERT/measured_utc: \"2026-01-01T00:00:00Z\"/}"
pb b_cert_nokey PASS --bench-report-src docs/benchmark/bench_report_26010120_self-test-model_GB10_0.0.0.md --certificate-src docs/benchmark/benchmark_26010120_self-test-model_GB10_0.0.0.yaml
rm -f "$ROOT"/docs/benchmark/*_26010120_*
pb b_refute REFUTE --bench-report-src "$REP_REL"
vj() { w "docs/_evidence/inputs/$1.json" "$2"; }
for a in explore weak explicit; do
  vj "v_$a" "{\"verdict\": \"PASS\", \"rubric\": {\"authority\": \"$a\", \"floor\": 8.5, \"ratio_M_over_primary\": 1.23, \"source\": \"expected_achievable(roofline×MBU)\"}}"
  pb "b_vj_$a" PASS --bench-report-src "$REP_REL" --verdict-json-src "docs/_evidence/inputs/v_$a.json"
done
vj v_badauth '{"verdict": "PASS", "rubric": {"authority": "bogus"}}'
pb b_vj_badauth PASS --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_badauth.json
vj v_mismatch '{"verdict": "REFUTE", "rubric": {"authority": "weak", "floor": true, "ratio_M_over_primary": 2}}'
pb b_vj_mismatch PASS --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_mismatch.json
pb b_vj_refute_ok REFUTE --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_mismatch.json
vj v_notjson '{"verdict": '
pb b_vj_notjson PASS --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_notjson.json
vj v_list '[1]'
pb b_vj_list PASS --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_list.json
vj v_strip '{"verdict": "PASS", "rubric": {"authority": "  explore  ", "source": "  ", "floor": 1e400}}'
pb b_vj_strip PASS --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_strip.json
pb b_pass_nocert PASS --bench-report-src "$REP_REL"
vj v_obs '{"verdict": "NEEDS_RUBRIC", "failure_axis": "establish", "reason_code": null, "rubric": {"authority": "weak", "primary": null, "source": null, "floor": null, "ratio_M_over_primary": null}}'
pb b_obs OBSERVATION-ONLY --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_obs.json
vj v_obs_phys '{"verdict": "NEEDS_RUBRIC", "reason_code": null, "rubric": {"primary": 59.0}}'
pb b_obs_phys OBSERVATION-ONLY --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_obs_phys.json
vj v_obs_spec '{"verdict": "NEEDS_RUBRIC", "reason_code": "SPEC_ACCEPT_LEN_MISSING", "rubric": {}}'
pb b_obs_spec OBSERVATION-ONLY --bench-report-src "$REP_REL" --verdict-json-src docs/_evidence/inputs/v_obs_spec.json
pb b_obs_nojson OBSERVATION-ONLY --bench-report-src "$REP_REL"
pb b_pass_rebind PASS --bench-report-src "$REP_REL" --certificate-src "$CERT_REL"
w docs/benchmark/bench_report_26010109_00_01_self-test-model_GB10_0.0.0.md $'# report 2\n'
pb b_report_rebound REFUTE --bench-report-src docs/benchmark/bench_report_26010109_00_01_self-test-model_GB10_0.0.0.md
pb b_badutc REFUTE --bench-report-src "$REP_REL" --generated-utc nope
st b_reinit init "${R[@]}" --topic bench --task-class full_benchmark --generated-utc 2026-01-01T02:00:00Z --identity-json identity.json
st b_reinit_verdict_rebind init "${R[@]}" --topic bench --task-class full_benchmark --generated-utc 2026-01-01T02:00:00Z --identity-json identity.json --benchmark-verdict PASS

# ── set-narrative binding ───────────────────────────────────────────────────────────────────────────────────
PLAN_SRC=docs/plan/plan_26010109_selftest_source.md
w "$PLAN_SRC" $'# plan\n실제 저작 본문.\n'
st bind_plan set-narrative "${R[@]}" --topic bench --kind plan --narrative-file "$PLAN_SRC" --author selftest --generated-utc 2026-01-01T02:20:00Z
w docs/_evidence/inputs/dev.md $'서사 본문\n'
st inject_dev set-narrative "${R[@]}" --topic bench --kind devlog --narrative-file docs/_evidence/inputs/dev.md --author selftest --generated-utc 2026-01-01T02:21:00Z
PH=docs/testlog/testlog_26010109_selftest_ph.md
w "$PH" $'_PLACEHOLDER -- agent-authored narrative pending. Do not treat as final evidence._\n'
st bind_placeholder set-narrative "${R[@]}" --topic bench --kind testlog --narrative-file "$PH" --author selftest --generated-utc 2026-01-01T02:22:00Z
rm -f "$ROOT/$PLAN_SRC"
st reinit_bound_missing init "${R[@]}" --topic bench --task-class full_benchmark --generated-utc 2026-01-01T02:00:00Z --identity-json identity.json

# ── downgrade (full_benchmark -> hint_map_only) ──────────────────────────────────────────────────────────────
MC_TITLE='## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)'
mc() { local utc="$1"; shift; printf '# report\n생성일 %s.\n\n%s\n\n| 키 | 값 |\n|---|---|\n' "$utc" "$MC_TITLE"; while [ "$#" -gt 0 ]; do printf '| %s | %s |\n' "$1" "$2"; shift 2; done; }
dinit() { local name="$1" topic="$2" cls="$3"; shift 3; st "$name" init "${R[@]}" --topic "$topic" --task-class "$cls" --generated-utc 2026-01-01T03:00:00Z --identity-json identity.json "$@"; }
full_topic() {
  local topic="$1" stem="$2" text="$3"
  w "docs/benchmark/bench_report_$stem.md" "$text"
  dinit "d_${topic}_init" "$topic" full_benchmark --benchmark-mode full --benchmark-verdict PASS
  st "d_${topic}_pub" publish-benchmark "${R[@]}" --topic "$topic" --verdict PASS --generated-utc 2026-01-01T03:05:00Z --bench-report-src "docs/benchmark/bench_report_$stem.md"
}
full_topic down 26010112_self-test-model_GB10_0.0.0 "$(mc 2026-01-01T03:00:00Z bench_mode lite bench_mode_kind downgraded-lite bench_mode_source '반복 조건 불성립(판정점 run 2 실패)' downgrade_reason run_failed)"
full_topic downfull 26010114_self-test-model_GB10_0.0.0 "$(mc 2026-01-01T03:10:00Z bench_mode full bench_mode_kind full bench_mode_source '반복 조건 성립' downgrade_reason N/A)"
full_topic downlegacy 26010115_self-test-model_GB10_0.0.0 $'# report\n생성일 2026-01-01T03:20:00Z.\n\n| 동시성 | decode t/s |\n|---|---|\n| 1 | 9 |\n'
full_topic downbad 26010117_self-test-model_GB10_0.0.0 "$(mc 2026-01-01T03:30:00Z bench_mode lite unknown_key x)"
full_topic downbad2 26010118_self-test-model_GB10_0.0.0 "$(printf '# r\n\n%s\n\n| a | b | c |\n' "$MC_TITLE")"
full_topic downempty 26010119_self-test-model_GB10_0.0.0 "$(printf '# r\n\n%s\n\nno table\n' "$MC_TITLE")"
dinit d_noreport_init downnoreport full_benchmark --benchmark-mode full --benchmark-verdict PASS
for spec in downfull:run_failed downlegacy:run_failed down:blackbox_kill downnoreport:run_failed downbad:run_failed downbad2:run_failed downempty:run_failed; do
  t="${spec%%:*}"; r="${spec#*:}"
  dinit "d_neg_${t}_${r}" "$t" hint_map_only --downgrade-from full_benchmark --downgrade-reason "$r"
done
dinit d_neg_plain down hint_map_only
dinit d_neg_noreason down hint_map_only --downgrade-from full_benchmark
dinit d_neg_variance down hint_map_only --downgrade-from full_benchmark --downgrade-reason variance
dinit d_neg_reason_only down hint_map_only --downgrade-reason run_failed
dinit d_neg_mode down hint_map_only --downgrade-from full_benchmark --downgrade-reason run_failed --benchmark-mode full
dinit d_neg_target down harness_change --downgrade-from full_benchmark --downgrade-reason run_failed
dinit d_neg_prior_absent nope hint_map_only --downgrade-from full_benchmark --downgrade-reason run_failed
dinit d_neg_from_choice down hint_map_only --downgrade-from harness_change --downgrade-reason run_failed
dinit d_ok down hint_map_only --downgrade-from full_benchmark --downgrade-reason run_failed
dinit d_ok_again down hint_map_only --downgrade-from full_benchmark --downgrade-reason run_failed
dinit d_ok_plain down hint_map_only
dinit d_revert down full_benchmark
dinit d_other_reason down hint_map_only --downgrade-from full_benchmark --downgrade-reason blackbox_kill
sed "s|\"certificate\": null|\"certificate\": \"$CERT_REL\"|" "$ROOT/docs/_evidence/bench.json" >"$ROOT/bench.tmp" && mv "$ROOT/bench.tmp" "$ROOT/docs/_evidence/bench.json"
st d_cert_bound init "${R[@]}" --topic bench --task-class hint_map_only --generated-utc 2026-01-01T02:00:00Z --identity-json identity.json --downgrade-from full_benchmark --downgrade-reason run_failed

# ── publish-lite-report + lite certificate + finalize + promotion target ────────────────────────────────────
LSTEM=26010113_self-test-model_GB10_0.0.0
LREP=docs/benchmark/bench_report_$LSTEM.md
w "$LREP" "$(printf '# 경량\n\nmode: lite\n\n'; mc 2026-01-01T04:00:00Z bench_mode lite bench_mode_kind declared-lite bench_mode_source 'declared(lite_bench · lite-only 셀)')"
dinit l_init lite hint_map_only
lb() { local name="$1"; shift; st "$name" publish-lite-report "${R[@]}" --topic lite --generated-utc 2026-01-01T04:05:00Z "$@"; }
lb l_full_report --bench-report-src "$REP_REL"
st l_wrong_class publish-lite-report "${R[@]}" --topic bench --generated-utc 2026-01-01T04:05:00Z --bench-report-src "$LREP"
w docs/benchmark/bench_report_26010116_self-test-model_GB10_0.0.0.md $'# 산문\n생성일 2026-01-01T04:01:00Z.\n\n| bench_mode | lite |\n'
lb l_stray --bench-report-src docs/benchmark/bench_report_26010116_self-test-model_GB10_0.0.0.md
lb l_table_full --bench-report-src docs/benchmark/bench_report_26010114_self-test-model_GB10_0.0.0.md
lb l_table_broken --bench-report-src docs/benchmark/bench_report_26010117_self-test-model_GB10_0.0.0.md
cp "$ROOT/$LREP" "$ROOT/docs/_evidence/inputs/lite.md"
lb l_noncanon --bench-report-src docs/_evidence/inputs/lite.md
lb l_ok --bench-report-src "$LREP"
LCERT=docs/benchmark/benchmark_$LSTEM.yaml
LCERT_TEXT=$'schema_version: 1\nrecord_type: benchmark_certificate\nverdict: not_applicable\nmodel: self-test-model\ngpu_model: GB10\nvllm_version: 0.0.0\nquantization: N/A\ntopology: single\ntensor_parallel_size: 1\nbenchmark_mode: lite\nlite_verdict: pass\nentry_path: beta_lite_only_cell\nlite_included: true\nlite_gen_tps_warm: 40.0\nmeasured_utc: "2026-01-01T04:00:00Z"\nmeasured_node: main\n'
w "$LCERT" "${LCERT_TEXT/lite_verdict: pass/lite_verdict: server_failed}"; lb l_cert_grade1 --bench-report-src "$LREP" --certificate-src "$LCERT"
w "$LCERT" "${LCERT_TEXT/benchmark_mode: lite/benchmark_mode: full}"; lb l_cert_grade2 --bench-report-src "$LREP" --certificate-src "$LCERT"
w "$LCERT" "${LCERT_TEXT/model: self-test-model/model: other-model}"; lb l_cert_identity --bench-report-src "$LREP" --certificate-src "$LCERT"
w "$LCERT" $'x: [1]\n'; lb l_cert_unparseable --bench-report-src "$LREP" --certificate-src "$LCERT"
OTHER=docs/benchmark/benchmark_26010115_self-test-model_GB10_0.0.0.yaml
rm -f "$ROOT/$LCERT"
w "$OTHER" "${LCERT_TEXT//04:00:00Z/05:00:00Z}"; lb l_cert_stem --bench-report-src "$LREP" --certificate-src "$OTHER"; rm -f "$ROOT/$OTHER"
w "$LCERT" "$LCERT_TEXT"; lb l_cert_ok --bench-report-src "$LREP" --certificate-src "$LCERT"
for k in plan devlog testlog; do
  w "docs/_evidence/inputs/lite_$k.md" "# $k"$'\n경량 셀 서사.\n'
  st "l_narr_$k" set-narrative "${R[@]}" --topic lite --kind "$k" --narrative-file "docs/_evidence/inputs/lite_$k.md" --author selftest --generated-utc 2026-01-01T04:06:00Z
done
PLAN_L="$(cd "$ROOT/docs/plan" && ls plan_*_lite.md)"; DEV_L="$(cd "$ROOT/docs/devlog" && ls devlog_*_lite.md)"; TEST_L="$(cd "$ROOT/docs/testlog" && ls testlog_*_lite.md)"
w pii_l.json "{\"passed\": true, \"scanned_paths\": [\"../benchmark/benchmark_$LSTEM.yaml\", \"../benchmark/bench_report_$LSTEM.md\", \"../devlog/$DEV_L\", \"../plan/$PLAN_L\", \"../testlog/$TEST_L\"]}"
st l_finalize finalize "${R[@]}" --topic lite --pii-scan-json pii_l.json --runtime-json runtime.json
A40=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
B40=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
TAG=hint/0.0.0/self-test-model/gb10-1g1n-main-native/qbf16-len1024-kvauto-plenone-specoff-graph
spt() { local name="$1"; shift; st "$name" set-promotion-target "${R[@]}" "$@"; }
spt p_short --topic lite --tag "$TAG" --topology "single TP=1" --anchor abc123 --generated-utc 2026-01-01T05:00:00Z
spt p_upper --topic lite --tag "$TAG" --topology "single TP=1" --anchor AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA --generated-utc 2026-01-01T05:00:00Z
spt p_release --topic lite --tag release/0.0.0 --topology "single TP=1" --anchor "$A40" --generated-utc 2026-01-01T05:00:00Z
spt p_glob --topic lite --tag 'hint/*' --topology "single TP=1" --anchor "$A40" --generated-utc 2026-01-01T05:00:00Z
spt p_multiline --topic lite --tag "$TAG" --topology $'multi\nTP=2' --anchor "$A40" --generated-utc 2026-01-01T05:00:00Z
spt p_padded --topic lite --tag "$TAG" --topology " single TP=1" --anchor "$A40" --generated-utc 2026-01-01T05:00:00Z
spt p_unknown --topic nope --tag "$TAG" --topology "single TP=1" --anchor "$A40" --generated-utc 2026-01-01T05:00:00Z
spt p_badutc --topic lite --tag "$TAG" --topology "single TP=1" --anchor "$A40" --generated-utc "2026-01-01 05:00"
spt p_bind --topic lite --tag "$TAG" --topology "single TP=1" --anchor "$A40" --generated-utc 2026-01-01T05:00:00Z
spt p_same --topic lite --tag "$TAG" --topology "single TP=1" --anchor "$A40" --generated-utc 2026-01-01T05:10:00Z
spt p_rebind --topic lite --tag "$TAG" --topology "single TP=1" --anchor "$B40" --generated-utc 2026-01-01T05:20:00Z
dinit p_reinit lite hint_map_only
st p_finalize finalize "${R[@]}" --topic lite --pii-scan-json pii_l.json --runtime-json runtime.json
sed 's/"promotion_target_provenance"/"zz_unrelated"/' "$ROOT/docs/_evidence/lite.json" >"$ROOT/lite.tmp" && mv "$ROOT/lite.tmp" "$ROOT/docs/_evidence/lite.json"
st p_hand finalize "${R[@]}" --topic lite

# ── simlog raw copies + PII exemption coverage ──────────────────────────────────────────────────────────────
dinit x_init piix full_benchmark --benchmark-mode full --benchmark-verdict REFUTE
for k in plan devlog testlog; do
  w "docs/_evidence/inputs/piix_$k.md" "# $k"$'\n면제 커버리지 픽스처 서사.\n'
  st "x_narr_$k" set-narrative "${R[@]}" --topic piix --kind "$k" --narrative-file "docs/_evidence/inputs/piix_$k.md" --author selftest --generated-utc 2026-01-01T06:00:00Z
done
w docs/_evidence/inputs/measured.json $'{"measurement_ok": true}\n'
st x_simlog append-raw "${R[@]}" --topic piix --kind simlog --src docs/_evidence/inputs/measured.json --recorded-utc 2026-01-01T06:01:00Z --dest-name level_01_measured.json
st x_simlog_collide append-raw "${R[@]}" --topic piix --kind simlog --src docs/_evidence/inputs/measured.json --recorded-utc 2026-01-01T06:01:00Z --dest-name level_01_measured.json
st x_simlog_badname append-raw "${R[@]}" --topic piix --kind simlog --src docs/_evidence/inputs/measured.json --recorded-utc 2026-01-01T06:01:00Z --dest-name ..
st x_simlog_default append-raw "${R[@]}" --topic piix --kind simlog --src docs/_evidence/inputs/measured.json --recorded-utc 2026-01-01T06:01:00Z
st x_pub publish-benchmark "${R[@]}" --topic piix --verdict REFUTE --generated-utc 2026-01-01T06:02:00Z --bench-report-src "$REP_REL"
SIM_X="$(cd "$ROOT/docs/simlog" && ls -d *_piix)"
PLAN_X="$(cd "$ROOT/docs/plan" && ls plan_*_piix.md)"; DEV_X="$(cd "$ROOT/docs/devlog" && ls devlog_*_piix.md)"; TEST_X="$(cd "$ROOT/docs/testlog" && ls testlog_*_piix.md)"
SCAN_X="\"../benchmark/bench_report_$STEM.md\", \"../devlog/$DEV_X\", \"../plan/$PLAN_X\", \"../testlog/$TEST_X\""
RAW='machine-raw(docs.md §PII 판정 대상 밖)'
w pii_x1.json "{\"passed\": true, \"scanned_paths\": [$SCAN_X], \"exempt_paths\": [{\"path\": \"../simlog/$SIM_X\", \"reason\": \"$RAW\"}]}"
st x_fin_exempt finalize "${R[@]}" --topic piix --pii-scan-json pii_x1.json --runtime-json runtime.json
w pii_x2.json "{\"passed\": true, \"scanned_paths\": [$SCAN_X], \"exempt_paths\": [{\"path\": \"../simlog/$SIM_X\", \"reason\": \"귀찮아서\"}]}"
st x_fin_badreason finalize "${R[@]}" --topic piix --pii-scan-json pii_x2.json --runtime-json runtime.json
w pii_x3.json "{\"passed\": true, \"scanned_paths\": [$SCAN_X], \"exempt_paths\": [{\"path\": \"/abs/../simlog/$SIM_X\", \"reason\": \"$RAW\"}]}"
st x_fin_abs finalize "${R[@]}" --topic piix --pii-scan-json pii_x3.json --runtime-json runtime.json

# ── symlinked write destinations ────────────────────────────────────────────────────────────────────────────
mkdir -p "$ROOT/elsewhere"
mv "$ROOT/docs/_evidence" "$ROOT/elsewhere/_evidence"; ln -s ../elsewhere/_evidence "$ROOT/docs/_evidence"
st sym_evidence_dir finalize "${R[@]}" --topic piix
rm -f "$ROOT/docs/_evidence"; mv "$ROOT/elsewhere/_evidence" "$ROOT/docs/_evidence"
mv "$ROOT/docs/plan" "$ROOT/elsewhere/plan"; ln -s ../elsewhere/plan "$ROOT/docs/plan"
st sym_plan_dir init "${R[@]}" --topic symp --task-class harness_change --generated-utc 2026-01-01T08:00:00Z --identity-json identity.json
rm -f "$ROOT/docs/plan"; mv "$ROOT/elsewhere/plan" "$ROOT/docs/plan"
st done_tree finalize "${R[@]}" --topic nar
