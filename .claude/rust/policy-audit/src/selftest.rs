//! `policy_registry.py --self-test`: an in-memory lifecycle round trip plus the selector-wiring negative controls.

use crate::arch::{slave_forward_groups, smoke_forwards_image_identity, COMPOSE_MULTI_TEMPLATE_REL, SLAVE_FORWARD_REL};
use crate::registry::evaluate_lifecycle;
use agent_runtime_support::pyjson;
use std::collections::BTreeSet;
use std::path::Path;

pub const OK_EVIDENCE_PATH: &str = ".claude/rust/runtime-support/src/schema.rs";
pub const OK_EVIDENCE_ID: &str = "validate_against_schema";

fn fail(msg: String) -> i32 {
    eprintln!("Traceback (most recent call last):\nRuntimeError: {msg}");
    1
}

fn fixture(pid: &str, bad: bool) -> String {
    if !bad {
        format!(
            r#"{{"policies": [{{"policy_id": "{pid}", "owner": "x", "statement": "s", "scope": ["single-node"], "origin_failure": "plan_x",
 "clauses": [{{"clause_id": "{pid}.C1", "statement": "mentions selftest keyword uniquely"}}],
 "evidence": [{{"path": "{OK_EVIDENCE_PATH}", "supports": ["{pid}.C1"], "assertion_ids": ["{OK_EVIDENCE_ID}"]}}],
 "remove_when": "Removable only once this self-test itself is deleted from the module.", "status": "candidate"}}]}}"#
        )
    } else {
        format!(
            r#"{{"policies": [{{"policy_id": "{pid}", "owner": "x", "statement": "s", "scope": ["single-node"], "origin_failure": "plan_x",
 "clauses": [{{"clause_id": "{pid}.C1", "statement": "x"}}],
 "evidence": [{{"path": "{OK_EVIDENCE_PATH}", "supports": ["{pid}.C1"], "assertion_ids": ["{OK_EVIDENCE_ID}"]}}],
 "remove_when": "xxxxxxxxxxxxxxxxxxxx", "status": "active"}}]}}"#
        )
    }
}

pub fn self_test(repo: &Path) -> i32 {
    let ok = pyjson::loads(&fixture("SELFTEST_OK", false)).unwrap();
    let v = evaluate_lifecycle(&ok, repo, None);
    if !v.is_empty() {
        let dicts: Vec<String> = v.iter().map(|x| x.to_dict().repr()).collect();
        return fail(format!("self-test OK fixture unexpectedly flagged: [{}]", dicts.join(", ")));
    }
    let bad = pyjson::loads(&fixture("SELFTEST_BAD", true)).unwrap();
    let v = evaluate_lifecycle(&bad, repo, None);
    let codes: BTreeSet<String> = v.iter().map(|x| x.reason_code.clone()).collect();
    let expected = ["LAZY_REMOVE_WHEN"];
    let missing: Vec<&str> = expected.iter().copied().filter(|c| !codes.contains(*c)).collect();
    if !missing.is_empty() {
        return fail(format!("self-test BAD fixture missing expected codes: {missing:?}, got {codes:?}"));
    }
    if let Err(m) = selector_wiring(repo) {
        return fail(m);
    }
    println!("[policy_registry] --self-test PASS");
    0
}

fn selector_wiring(repo: &Path) -> Result<(), String> {
    let wired = "SF=\"$SDIR/slave_forward.sh\"\nSFA=(--cell-env \"$EF\" --compose \"$COMPOSE_MULTI\")\nSLAVE_IMGVARS=\"$(\"$SF\" prefix --group image_identity,cluster \"${SFA[@]}\")\"\n";
    let retired = "IMG=$(val IMAGE_TAG); SMPORT=$(val SM12X_PORT)\nSLAVE_IMGVARS=\"${IMG:+IMAGE_TAG=$IMG }${SMPORT:+SM12X_PORT=$SMPORT }\"\n";
    if !smoke_forwards_image_identity(wired) {
        return Err("self-test: the slave_forward wiring must satisfy check (2a)".into());
    }
    for (label, text) in [
        ("the retired hand list", retired.to_string()),
        ("a derivation fed another env file", wired.replace("\"$EF\"", "\"$EFC\"")),
        ("a derivation of the cluster group only", wired.replace("image_identity,cluster", "cluster")),
    ] {
        if smoke_forwards_image_identity(&text) {
            return Err(format!("self-test: {label} must NOT satisfy check (2a)"));
        }
    }
    let rels = [SLAVE_FORWARD_REL, COMPOSE_MULTI_TEMPLATE_REL];
    let td = tempdir().map_err(|e| e.to_string())?;
    let root = td.path();
    let result = (|| -> Result<(), String> {
        for rel in rels {
            let dst = root.join(rel);
            std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::copy(repo.join(rel), &dst).map_err(|e| format!("{e}: {}", repo.join(rel).display()))?;
        }
        let groups = slave_forward_groups(root);
        match &groups {
            Ok(g) if g.iter().any(|k| k == "SM12X_PORT") => {}
            other => return Err(format!("self-test: the compose template must derive SM12X_PORT as image identity ({:?})", other.as_ref().err())),
        }
        let compose = root.join(COMPOSE_MULTI_TEMPLATE_REL);
        let text = std::fs::read_to_string(&compose).map_err(|e| e.to_string())?;
        let dropped: String = text.split_inclusive('\n').filter(|ln| !ln.trim_start().starts_with("SM12X_PORT:")).collect();
        if dropped == text {
            return Err("self-test: the compose template no longer declares the SM12X_PORT build-arg line".into());
        }
        std::fs::write(&compose, dropped).map_err(|e| e.to_string())?;
        match slave_forward_groups(root) {
            Ok(g) if !g.iter().any(|k| k == "SM12X_PORT") => {}
            other => return Err(format!("self-test: a compose without the build-arg must not derive it ({:?})", other.err())),
        }
        std::fs::remove_file(root.join(SLAVE_FORWARD_REL)).map_err(|e| e.to_string())?;
        match slave_forward_groups(root) {
            Err(why) if !why.is_empty() => Ok(()),
            _ => Err("self-test: an absent slave_forward owner must be 'cannot judge' with a reason".into()),
        }
    })();
    result
}

pub struct TempDir(std::path::PathBuf);
impl TempDir {
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn tempdir() -> std::io::Result<TempDir> {
    let base = std::env::temp_dir();
    for n in 0..1000u32 {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let p = base.join(format!("policy-registry-sf-{}-{}-{n}", std::process::id(), nanos));
        match std::fs::create_dir(&p) {
            Ok(()) => return Ok(TempDir(p)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(std::io::ErrorKind::Other, "no temp dir"))
}
