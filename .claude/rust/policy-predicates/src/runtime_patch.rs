//! `runtime_patch_predicate.py` (unittest): RUNTIME_PATCH_NO_CARRY_FORWARD provenance. The production validator is
//! driven through its command line, which is the Rust `validate_runtime_patch` behind `validate_runtime_patch.sh`
//! (the original ran `python3 validate_runtime_patch.py`, whose command line that launcher replaced).
use crate::support::*;
use crate::unit::Case;
use std::path::{Path, PathBuf};

fn validator() -> PathBuf {
    repo_root().join(".claude/skills/upstream-version-watch/scripts/validate_runtime_patch.sh")
}
fn resolution() -> String {
    repo_root().join(".claude/skills/upstream-version-watch/assets/current-production-resolution.json").to_string_lossy().into_owned()
}

fn call(action: &str, patch: &Path, configs: &Path, envs: &Path) -> R<Proc> {
    let (v, p, c, e, r) = (validator().to_string_lossy().into_owned(), patch.to_string_lossy().into_owned(),
                           configs.to_string_lossy().into_owned(), envs.to_string_lossy().into_owned(), resolution());
    run(&["bash", &v, action, "--patch", &p, "--topology", "multi", "--config-dir", &c, "--env-dir", &e, "--resolution", &r], None, &[], None, None)
}

fn sha256_hex(bytes: &[u8]) -> R<String> {
    use sha2::Digest;
    Ok(sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect())
}

fn test_validator_stamps_and_rejects_every_stale_dimension() -> R {
    req(validator().is_file(), "False is not true")?;
    let tmp = TempDir::new("tmp")?;
    let (configs, envs) = (tmp.path().join("configs"), tmp.path().join("envs"));
    mkdirs(&configs)?;
    mkdirs(&envs)?;
    let patch = configs.join("modelA_patch.py");
    write(&patch, "# current patch\n")?;
    write(&configs.join("modelA.yaml"), "model: A\nversion: 1\n")?;
    write(&envs.join(".env.modelA"), "MODEL=A\n")?;
    let stamp = call("stamp", &patch, &configs, &envs)?;
    req(stamp.code == 0, format!("{} != 0 : {}", stamp.code, stamp.stderr))?;
    let verify = call("verify", &patch, &configs, &envs)?;
    req(verify.code == 0, format!("{} != 0 : {}", verify.code, verify.stderr))?;
    let sidecar = configs.join("modelA_patch.provenance.json");
    let data = json_loads(&read_path(&sidecar)?)?;
    let want = sha256_hex(&std::fs::read(&patch).map_err(|e| Fail::new("OSError", e.to_string()))?)?;
    let got = jget(&data, "patch_sha256").ok().and_then(jstr).unwrap_or_default();
    req(got == want, format!("{} != {}", repr(&got), repr(&want)))?;
    write(&patch, "# stale carried-forward patch\n")?;
    req(call("verify", &patch, &configs, &envs)?.code != 0, "0 == 0")?;
    write(&patch, "# current patch\n")?;
    req(call("stamp", &patch, &configs, &envs)?.code == 0, "stamp != 0")?;
    write(&configs.join("modelA.yaml"), "model: A\nversion: 2\n")?;
    req(call("verify", &patch, &configs, &envs)?.code != 0, "0 == 0")?;
    write(&configs.join("modelA.yaml"), "model: A\nversion: 1\n")?;
    req(call("stamp", &patch, &configs, &envs)?.code == 0, "stamp != 0")?;
    // Sidecar topology laundering: rewrite the "topology" value to "single" (json.dumps of the loaded dict).
    let text = read_path(&sidecar)?;
    let laundered = re("(\"topology\"\\s*:\\s*)\"[^\"]*\"").replace(&text, "${1}\"single\"").into_owned();
    req(laundered != text, "the stamped sidecar must carry a topology field")?;
    write(&sidecar, &laundered)?;
    req(call("verify", &patch, &configs, &envs)?.code != 0, "0 == 0")
}

fn test_missing_sidecar_and_orphan_patch_fail_closed() -> R {
    let tmp = TempDir::new("tmp")?;
    let (configs, envs) = (tmp.path().join("configs"), tmp.path().join("envs"));
    mkdirs(&configs)?;
    mkdirs(&envs)?;
    let patch = configs.join("stale_model_patch.py");
    write(&patch, "# stale provenance-free carried-forward patch\n")?;
    req(call("verify", &patch, &configs, &envs)?.code != 0, "0 == 0")
}

fn test_sync_delivery_calls_production_validator_and_transfers_sidecar() -> R {
    let source = read(".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh")?;
    for needle in ["validate_runtime_patches", "validate_runtime_patches \"$1\"", "*_patch.provenance.json"] {
        req(source.contains(needle), format!("{} not found in source", repr(needle)))?;
    }
    let a = index(&source, "validate_runtime_patches \"$1\"")?;
    let b = index_from(&source, "_band2_filters", index(&source, "deliver_build()")?)?;
    req(a < b, format!("{a} not less than {b}"))
}

fn test_canonical_claim_bindings_are_production_owned() -> R {
    let doc = json_load(".claude/policies/claim_bindings.json")?;
    let bindings = jget(&doc, "bindings")?;
    let paths = |cid: &str| -> R<Vec<String>> {
        Ok(jlist(jget(bindings, cid)?).iter().filter_map(|e| jget(e, "path").ok().and_then(jstr)).collect())
    };
    let p = paths("TERRAFORM_FLAG_GATE.C4")?;
    req(p.iter().any(|x| x == ".claude/skills/wiki-desk/src/cli.ts"), "'.claude/skills/wiki-desk/src/cli.ts' not found in paths")?;
    req(!p.is_empty(), "set() is not true")?;
    req(p.iter().all(|x| !x.starts_with("tests/") && !x.starts_with("scripts/")), "False is not true")?;
    let c2 = paths("RUNTIME_PATCH_NO_CARRY_FORWARD.C2")?;
    // The production validator is the upstream runtime's Rust `runtime_patch` module (validate_runtime_patch.sh starts
    // it); the `.py` it replaced is a reference original (plan_26111615 pass 3 re-pointed the evidence).
    req(c2.iter().any(|x| x == ".claude/skills/upstream-version-watch/rust/runtime/src/runtime_patch.rs"),
        "'.claude/skills/upstream-version-watch/rust/runtime/src/runtime_patch.rs' not found in c2")
}

pub fn cases() -> Vec<Case> {
    let m = "runtime_patch_predicate";
    let c = "RuntimePatchProvenanceTest";
    vec![
        Case { module: m, class: c, name: "test_canonical_claim_bindings_are_production_owned", f: test_canonical_claim_bindings_are_production_owned },
        Case { module: m, class: c, name: "test_missing_sidecar_and_orphan_patch_fail_closed", f: test_missing_sidecar_and_orphan_patch_fail_closed },
        Case { module: m, class: c, name: "test_sync_delivery_calls_production_validator_and_transfers_sidecar", f: test_sync_delivery_calls_production_validator_and_transfers_sidecar },
        Case { module: m, class: c, name: "test_validator_stamps_and_rejects_every_stale_dimension", f: test_validator_stamps_and_rejects_every_stale_dimension },
    ]
}
