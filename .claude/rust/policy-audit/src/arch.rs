//! `arch_variant_contract_violations` -- the fail-closed contract of the arch-wall variant ledger and its promotion
//! procedure (`policy_registry.py`). The slave-forward derivation the selector check runs is the upstream owner's Rust
//! library (`upstream_runtime::slave_forward::candidates`), gated on the same two owner files the original loaded.

use crate::registry::{read_text, resolve_evidence_path, safe_load_json_object, Violation};
use crate::value::*;
use agent_runtime_support::pyvalue::PyValue;
use fancy_regex::Regex;
use std::collections::BTreeSet;
use std::path::Path;

/// The owner of the derivation: the upstream Rust module this check calls in-process (the original gated on its
/// `slave_forward.py`; that program is now this module plus the `slave_forward.sh` launcher).
pub const SLAVE_FORWARD_REL: &str = ".claude/skills/upstream-version-watch/rust/runtime/src/slave_forward.rs";
pub const COMPOSE_MULTI_TEMPLATE_REL: &str = ".claude/skills/upstream-version-watch/templates/docker-compose.multi.template.yaml";
const LEDGER: &str = ".claude/policies/arch_variant_ledger.json";
const STOCK_REPO: &str = "https://github.com/vllm-project/vllm.git";

fn full(pattern: &str, text: &str) -> bool {
    Regex::new(&format!(r"\A(?:{pattern})\z")).expect("static regex").is_match(text).unwrap_or(false)
}

fn starts(pattern: &str, text: &str) -> bool {
    Regex::new(&format!(r"\A(?:{pattern})")).expect("static regex").is_match(text).unwrap_or(false)
}

/// `(groups, reason)` of the derivation over THIS repo_root's compose template; `Err(reason)` = cannot judge.
pub fn slave_forward_groups(repo_root: &Path) -> Result<Vec<String>, String> {
    for rel in [SLAVE_FORWARD_REL, COMPOSE_MULTI_TEMPLATE_REL] {
        if !std::fs::metadata(repo_root.join(rel)).map(|m| m.is_file()).unwrap_or(false) {
            return Err(format!("{rel} is absent"));
        }
    }
    let compose = repo_root.join(COMPOSE_MULTI_TEMPLATE_REL);
    match upstream_runtime::slave_forward::candidates(&compose.to_string_lossy(), None) {
        Err(e) => Err(format!("slave_forward.candidates failed: ForwardError: {}: {}", e.code, e.message)),
        Ok(groups) => match groups.iter().find(|(g, _)| *g == "image_identity") {
            Some((_, keys)) => Ok(keys.clone()),
            None => Err("slave_forward.candidates returned no image_identity group".into()),
        },
    }
}

/// (2a) text wiring of the multi smoke.
pub fn smoke_forwards_image_identity(smoke: &str) -> bool {
    let imgvars = crate::pyids::py_splitlines(smoke).into_iter().find(|ln| crate::pyids::py_strip(ln).starts_with("SLAVE_IMGVARS=")).unwrap_or("");
    smoke.contains("SF=\"$SDIR/slave_forward.sh\"") && smoke.contains("--cell-env \"$EF\"") && imgvars.contains("\"$SF\" prefix --group image_identity")
}

fn str_of(v: &PyValue) -> Option<String> {
    as_str(v)
}

pub fn arch_variant_contract_violations(repo_root: &Path) -> Vec<Violation> {
    let mut out: Vec<Violation> = Vec::new();
    let arch_pid = s("ARCH_WALL_VARIANT_LADDER");
    let fail = |out: &mut Vec<Violation>, code: &str, message: String, path: String| {
        out.push(Violation::new(code, message).pid(&arch_pid).path_s(path));
    };

    let resolved = match safe_load_json_object(&repo_root.join(LEDGER), "ARCH_VARIANT_LEDGER") {
        Ok(d) => d,
        Err(e) => {
            fail(&mut out, "ARCH_VARIANT_LEDGER_INVALID", e.message, "resolved.json".into());
            return out;
        }
    };
    let rd = dict(&resolved).unwrap();
    let root_keys = keys(rd);
    let root_meta: BTreeSet<String> = root_keys.iter().filter(|k| k.starts_with('_')).cloned().collect();
    let rest: BTreeSet<String> = root_keys.iter().filter(|k| !root_meta.contains(*k)).cloned().collect();
    let want: BTreeSet<String> = ["schema_version", "source_build_variants"].iter().map(|x| x.to_string()).collect();
    let sv = get(&resolved, "schema_version");
    if rest != want || !(eq_int(sv, 1) || eq_int(sv, 2) || eq_int(sv, 3)) {
        fail(
            &mut out,
            "ARCH_VARIANT_LEDGER_SHAPE_INVALID",
            "ledger requires schema_version in {1,2,3}, source_build_variants, and only scalar underscore metadata besides".into(),
            LEDGER.into(),
        );
    }
    for key in &root_meta {
        if !nonblank_str(get(&resolved, key)) {
            fail(&mut out, "ARCH_VARIANT_LEDGER_SHAPE_INVALID", format!("root metadata key {} must be a non-empty string", r(key)), format!("{LEDGER}.{key}"));
        }
    }
    let variants = get(&resolved, "source_build_variants").clone();
    let Some(vd) = dict(&variants) else {
        fail(
            &mut out,
            "ARCH_VARIANT_LEDGER_MISSING",
            format!("{LEDGER}.source_build_variants must be an object"),
            format!("{LEDGER}.source_build_variants"),
        );
        return out;
    };

    let dockerfile_rel = ".claude/skills/upstream-version-watch/templates/Dockerfile.source-build.template";
    let smoke_rel = ".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh";
    let dockerfile = match read_text(&repo_root.join(dockerfile_rel)) {
        Ok(t) => t,
        Err(e) => {
            fail(&mut out, "ARCH_VARIANT_DOCKERFILE_UNREADABLE", e, dockerfile_rel.into());
            String::new()
        }
    };
    let smoke = read_text(&repo_root.join(smoke_rel)).unwrap_or_default();
    let mut sf_state: Option<Result<Vec<String>, String>> = None;

    let validate_bound_artifact = |out: &mut Vec<Violation>, name: &str, item: &PyValue, field: &str, kind: &str, pfx: &str| {
        let pointer = get(item, field);
        let pointer_ok = dict(pointer).map_or(false, |d| keys(d) == vec!["path".to_string()]);
        if !pointer_ok {
            fail(out, "ARCH_VARIANT_ARTIFACT_POINTER_INVALID", format!("{} {field} must be an exact single-key path object", r(name)), format!("{pfx}.{field}"));
            return;
        }
        let rel_v = get(pointer, "path");
        let Some(rel) = str_of(rel_v) else {
            fail(out, "ARCH_VARIANT_ARTIFACT_PATH_INVALID", format!("{} {field} requires a path string", r(name)), format!("{pfx}.{field}.path"));
            return;
        };
        let status = resolve_evidence_path(repo_root, rel_v);
        if status != "ok" {
            fail(
                out,
                "ARCH_VARIANT_ARTIFACT_UNBOUND",
                format!("{} {field} is not a regular, git-tracked, undrifted file (resolution={status})", r(name)),
                format!("{pfx}.{field}.path"),
            );
            return;
        }
        let artifact = match safe_load_json_object(&repo_root.join(&rel), "ARCH_VARIANT_ARTIFACT") {
            Ok(a) => a,
            Err(e) => {
                fail(out, "ARCH_VARIANT_ARTIFACT_INVALID", e.message, format!("{pfx}.{field}.path"));
                return;
            }
        };
        let mut fields: BTreeSet<String> =
            ["schema_version", "kind", "result", "variant_id", "vllm_repo", "vllm_ref", "track", "architecture", "image_tag"].iter().map(|x| x.to_string()).collect();
        if kind == "arch_variant_approval" {
            fields.extend(["approved_by", "source_evidence", "source_path", "approved_scope"].iter().map(|x| x.to_string()));
        } else {
            fields.insert("existing_models".into());
        }
        let akeys: BTreeSet<String> = keys(dict(&artifact).unwrap()).into_iter().collect();
        if !eq_int(get(&artifact, "schema_version"), 1) || akeys != fields {
            fail(
                out,
                "ARCH_VARIANT_ARTIFACT_SHAPE_INVALID",
                format!("{} {field} must match the closed schema_version=1 {kind} shape", r(name)),
                format!("{pfx}.{field}.path"),
            );
        }
        let expected: Vec<(&str, PyValue)> = vec![
            ("kind", s(kind)),
            ("result", s("PASS")),
            ("variant_id", get(item, "variant_id").clone()),
            ("vllm_repo", get(item, "vllm_repo").clone()),
            ("vllm_ref", get(item, "vllm_ref").clone()),
            ("track", get(item, "track").clone()),
            ("architecture", get(item, "architecture").clone()),
            ("image_tag", get(item, "image_tag").clone()),
        ];
        if expected.iter().any(|(k, v)| get(&artifact, k) != v) {
            fail(
                out,
                "ARCH_VARIANT_ARTIFACT_BINDING_MISMATCH",
                format!("{} {field} must PASS and bind the exact variant/ref/track/architecture/image", r(name)),
                format!("{pfx}.{field}.path"),
            );
        }
        if kind == "arch_variant_approval" {
            let approved_by = str_of(get(&artifact, "approved_by"));
            let plan_id: Option<String> = approved_by.as_deref().and_then(|a| {
                if full(r"HITL:(plan_\d{8})", a) {
                    Some(a["HITL:".len()..].to_string())
                } else {
                    None
                }
            });
            if plan_id.is_none() {
                fail(out, "ARCH_VARIANT_APPROVAL_IDENTITY_MISSING", format!("{} approval lacks explicit HITL identity", r(name)), format!("{pfx}.{field}.path"));
            }
            let plan_id = plan_id.unwrap_or_default();
            let expected_source = format!("{plan_id} §3 R1 primary-source fork inspection and same-hardware community evidence");
            let expected_scope = "historical 0.23.0 rollback variant; serving smoke remains final arbitration";
            let source_ok = str_of(get(&artifact, "source_evidence")).as_deref() == Some(expected_source.as_str());
            let expected_source_path = ".claude/policies/provenance/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md";
            let source_path = get(&artifact, "source_path");
            let source_path_value = str_of(source_path).unwrap_or_default();
            let source_status = if source_path_value.is_empty() { "absent" } else { resolve_evidence_path(repo_root, &s(&source_path_value)) };
            let source_text = if source_status == "ok" { read_text(&repo_root.join(&source_path_value)).unwrap_or_default() } else { String::new() };
            let source_artifact_ok = source_path_value == expected_source_path
                && str_of(source_path).is_some()
                && source_status == "ok"
                && source_text.contains("## 3. R1 해소 — 비-repack MoE 경로")
                && source_text.contains("포크 소스를 직독해 **확정**")
                && source_text.contains("**HUMMING** (fused grouped MoE)");
            let scope_ok = str_of(get(&artifact, "approved_scope")).as_deref() == Some(expected_scope);
            if !source_ok || !source_artifact_ok || !scope_ok {
                fail(
                    out,
                    "ARCH_VARIANT_APPROVAL_GROUNDING_MISSING",
                    format!("{} approval must bind the HITL plan section/rationale and final-arbitration scope", r(name)),
                    format!("{pfx}.{field}.path"),
                );
            }
        }
        if kind == "arch_variant_regression" {
            let models = get(&artifact, "existing_models");
            let ok = matches!(models.peel(), PyValue::List(l) if !l.is_empty() && l.iter().all(nonblank_str));
            if !ok {
                fail(
                    out,
                    "ARCH_VARIANT_REGRESSION_MODELS_MISSING",
                    format!("{} regression requires a non-empty list of model result identities", r(name)),
                    format!("{pfx}.{field}.path"),
                );
            }
        }
    };

    let validate_candidate_evidence = |out: &mut Vec<Violation>, name: &str, item: &PyValue, pfx: &str| {
        let fail = |out: &mut Vec<Violation>, code: &str, message: String, path: String| {
            out.push(Violation::new(code, message).pid(&s("ARCH_WALL_VARIANT_LADDER")).path_s(path));
        };
        let pointer = get(item, "evidence");
        let ok = dict(pointer).map_or(false, |d| {
            let k: BTreeSet<String> = keys(d).into_iter().collect();
            k == ["plan", "stock_infeasible_testlogs"].iter().map(|x| x.to_string()).collect::<BTreeSet<_>>()
        });
        if !ok {
            fail(
                out,
                "ARCH_VARIANT_CANDIDATE_EVIDENCE_INVALID",
                format!("{} candidate evidence must be an exact plan/stock_infeasible_testlogs object", r(name)),
                format!("{pfx}.evidence"),
            );
            return;
        }
        let plan_ok = str_of(get(pointer, "plan")).map_or(false, |p| full(r"docs/plan/plan_\d{8}(?:_\d{2}_\d{2})?_[^/]+\.md", &p));
        if !plan_ok {
            fail(
                out,
                "ARCH_VARIANT_CANDIDATE_EVIDENCE_INVALID",
                format!("{} candidate evidence requires a docs/plan naming-SSOT plan path", r(name)),
                format!("{pfx}.evidence.plan"),
            );
        }
        let logs = get(pointer, "stock_infeasible_testlogs");
        let logs_ok = matches!(logs.peel(), PyValue::List(l) if !l.is_empty()
            && l.iter().all(|e| str_of(e).map_or(false, |t| full(r"docs/testlog/testlog_\d{8}(?:_\d{2}_\d{2})?_[^/]+\.md", &t))));
        if !logs_ok {
            fail(
                out,
                "ARCH_VARIANT_CANDIDATE_EVIDENCE_INVALID",
                format!("{} candidate requires at least one docs/testlog path evidencing stock structural infeasibility", r(name)),
                format!("{pfx}.evidence.stock_infeasible_testlogs"),
            );
        }
    };

    let mut validate_build_patch_selectors = |out: &mut Vec<Violation>, name: &str, selectors: &PyValue, pfx: &str| {
        let fail = |out: &mut Vec<Violation>, code: &str, message: String, path: String| {
            out.push(Violation::new(code, message).pid(&s("ARCH_WALL_VARIANT_LADDER")).path_s(path));
        };
        let Some(sd) = dict(selectors).filter(|d| !d.is_empty()) else {
            fail(
                out,
                "ARCH_VARIANT_PORT_SELECTORS_INVALID",
                format!("{} build_patch_selectors must be a non-empty build-arg object", r(name)),
                format!("{pfx}.build_patch_selectors"),
            );
            return;
        };
        let mut args = keys(sd);
        args.sort();
        for arg in args {
            let value = get(selectors, &arg);
            let sub = format!("{pfx}.build_patch_selectors.{arg}");
            if !full(r"[A-Z][A-Z0-9_]*", &arg) || !nonblank_str(value) {
                fail(
                    out,
                    "ARCH_VARIANT_PORT_SELECTORS_INVALID",
                    format!("{} selector {} must be an uppercase build-arg name with a non-empty string value", r(name), r(&arg)),
                    sub,
                );
                continue;
            }
            if str_of(value).as_deref() == Some("0") {
                fail(
                    out,
                    "ARCH_VARIANT_PORT_SELECTOR_NOT_ENABLING",
                    format!("{} selector {}=0 equals the stock default and creates no variant", r(name), r(&arg)),
                    sub.clone(),
                );
            }
            if !dockerfile.contains(&format!("ARG {arg}=0")) {
                fail(
                    out,
                    "ARCH_VARIANT_PORT_SELECTOR_UNGATED",
                    format!("{} selector {} lacks a default-off `ARG {arg}=0` gate in the source template", r(name), r(&arg)),
                    sub.clone(),
                );
            }
            if smoke.is_empty() {
                fail(
                    out,
                    "ARCH_VARIANT_PORT_SELECTOR_WIRING_UNREADABLE",
                    format!("{} selector {} cannot be proven cluster-wide: {smoke_rel} is unreadable", r(name), r(&arg)),
                    sub,
                );
                continue;
            }
            if !smoke_forwards_image_identity(&smoke) {
                fail(
                    out,
                    "ARCH_VARIANT_PORT_SELECTOR_NOT_CLUSTER_WIDE",
                    format!(
                        "{} selector {} is not propagated to the slave as image identity: SLAVE_IMGVARS is not assigned from slave_forward.sh (image_identity) fed the model env file ($EF)",
                        r(name),
                        r(&arg)
                    ),
                    sub,
                );
                continue;
            }
            if sf_state.is_none() {
                sf_state = Some(slave_forward_groups(repo_root));
            }
            match sf_state.as_ref().unwrap() {
                Err(why) => {
                    fail(
                        out,
                        "ARCH_VARIANT_PORT_SELECTOR_WIRING_UNREADABLE",
                        format!("{} selector {} cannot be proven cluster-wide: {why}", r(name), r(&arg)),
                        sub,
                    );
                    continue;
                }
                Ok(identity) => {
                    if !identity.contains(&arg) {
                        fail(
                            out,
                            "ARCH_VARIANT_PORT_SELECTOR_NOT_CLUSTER_WIDE",
                            format!(
                                "{} selector {} is not derived by slave_forward.sh as cluster-wide image identity from the multi compose template (the compose must pass it as a build-arg)",
                                r(name),
                                r(&arg)
                            ),
                            sub,
                        );
                    }
                }
            }
        }
    };

    let validate_port_manifest = |out: &mut Vec<Violation>, name: &str, pm: &PyValue, pfx: &str| {
        let fail = |out: &mut Vec<Violation>, code: &str, message: String, path: String| {
            out.push(Violation::new(code, message).pid(&s("ARCH_WALL_VARIANT_LADDER")).path_s(path));
        };
        let Some(pd) = dict(pm) else {
            fail(out, "ARCH_VARIANT_PORT_MANIFEST_INVALID", format!("{} port_manifest must be an object", r(name)), format!("{pfx}.port_manifest"));
            return;
        };
        let required = ["files", "path", "upstream_base_sha"];
        let optional = ["fork_head_sha", "pr_base_sha"];
        let ks: BTreeSet<String> = keys(pd).into_iter().collect();
        let missing = required.iter().any(|k| !ks.contains(*k));
        let unknown = ks.iter().any(|k| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()));
        if missing || unknown {
            fail(
                out,
                "ARCH_VARIANT_PORT_MANIFEST_INVALID",
                format!("{} port_manifest requires {} and allows only {}", r(name), list_repr(required), list_repr(optional)),
                format!("{pfx}.port_manifest"),
            );
        }
        let path_ok = str_of(get(pm, "path")).map_or(false, |p| full(r"output/(?:single|multi)/build_patches_src/PROVENANCE\.json", &p));
        if !path_ok {
            fail(
                out,
                "ARCH_VARIANT_PORT_MANIFEST_INVALID",
                format!("{} port_manifest.path must be the pre-slot PROVENANCE.json inside a topology output lane", r(name)),
                format!("{pfx}.port_manifest.path"),
            );
        }
        let mut all: Vec<&str> = required.iter().chain(optional.iter()).copied().collect();
        all.sort();
        for field in all {
            if field.ends_with("_sha") && ks.contains(field) {
                let ok = str_of(get(pm, field)).map_or(false, |v| full(r"[0-9a-f]{40}", &v));
                if !ok {
                    fail(
                        out,
                        "ARCH_VARIANT_PORT_MANIFEST_INVALID",
                        format!("{} port_manifest.{field} must be a 40-hex commit SHA", r(name)),
                        format!("{pfx}.port_manifest.{field}"),
                    );
                }
            }
        }
        let files_ok = matches!(get(pm, "files").peel(), PyValue::Int(i) if *i > num_bigint::BigInt::from(0));
        if !files_ok {
            fail(out, "ARCH_VARIANT_PORT_MANIFEST_INVALID", format!("{} port_manifest.files must be a positive integer file count", r(name)), format!("{pfx}.port_manifest.files"));
        }
    };

    let metadata_keys = ["_note", "_deprecation_0.24.0"];
    let mut promoted: Vec<(String, PyValue)> = Vec::new();
    let mut candidates: Vec<(String, PyValue)> = Vec::new();
    let mut all_tagged: Vec<(String, String)> = Vec::new();
    let allowed_entry_fields = [
        "variant_id", "architecture", "vllm_repo", "vllm_ref", "tag", "image_tag", "track", "evidence", "status", "regression_evidence", "build_patch_selectors",
        "port_manifest",
    ];
    for (name_ps, item) in vd.iter() {
        let name = name_ps.to_utf8().unwrap_or_default();
        if name.starts_with('_') {
            if !metadata_keys.contains(&name.as_str()) || !nonblank_str(item) {
                fail(&mut out, "ARCH_VARIANT_METADATA_KEY_INVALID", format!("{} is not an allowlisted scalar metadata key", r(&name)), format!("{LEDGER}.source_build_variants.{name}"));
            }
            continue;
        }
        let pfx = format!("{LEDGER}.source_build_variants.{name}");
        let Some(id) = dict(item) else {
            fail(&mut out, "ARCH_VARIANT_ENTRY_INVALID", format!("{} must be an object", r(&name)), pfx);
            continue;
        };
        let ikeys = keys(id);
        let entry_meta: Vec<String> = {
            let mut m: Vec<String> = ikeys.iter().filter(|k| k.starts_with('_')).cloned().collect();
            m.sort();
            m
        };
        let mut unknown: Vec<String> = ikeys.iter().filter(|k| !allowed_entry_fields.contains(&k.as_str()) && !k.starts_with('_')).cloned().collect();
        unknown.sort();
        if !unknown.is_empty() {
            fail(&mut out, "ARCH_VARIANT_ENTRY_UNKNOWN_FIELD", format!("{} contains unknown fields {}", r(&name), list_repr(&unknown)), pfx.clone());
        }
        for key in &entry_meta {
            if !nonblank_str(get(item, key)) {
                fail(&mut out, "ARCH_VARIANT_ENTRY_UNKNOWN_FIELD", format!("{} metadata key {} must be a non-empty string", r(&name), r(key)), format!("{pfx}.{key}"));
            }
        }
        for field in ["variant_id", "architecture", "vllm_repo", "vllm_ref", "image_tag", "track", "status"] {
            if !nonblank_str(get(item, field)) {
                fail(&mut out, "ARCH_VARIANT_FIELD_MISSING", format!("{} requires non-empty {field}", r(&name)), format!("{pfx}.{field}"));
            }
        }
        // item.get(f, "") -- missing key -> "", present non-str -> not a str
        let field_or_empty = |f: &str| -> Option<String> {
            if has(item, f) {
                str_of(get(item, f))
            } else {
                Some(String::new())
            }
        };
        let track = field_or_empty("track");
        let architecture = field_or_empty("architecture");
        let image_tag = field_or_empty("image_tag").unwrap_or_default();
        let suffix = track.as_deref().map(|t| t.strip_prefix("source-").unwrap_or(t).to_string()).unwrap_or_default();
        let port_track = suffix.ends_with("-port");
        let fork_track = full(r"fork\d+", &suffix);
        let arch_suffix = suffix.strip_suffix("-port").unwrap_or(&suffix).to_string();
        if fork_track {
            let arch_ok = architecture.as_deref().map_or(false, |a| full(r"sm\d+[a-z0-9]*", a));
            if !arch_ok {
                let arch_repr = if has(item, "architecture") { get(item, "architecture").repr() } else { r("") };
                fail(
                    &mut out,
                    "ARCH_VARIANT_FIELD_MISSING",
                    format!("{} fork-pin track still requires a real architecture (sm<n>…), got {arch_repr}", r(&name)),
                    format!("{pfx}.architecture"),
                );
            }
            if !image_tag.ends_with(&format!("-source-{suffix}")) {
                fail(&mut out, "ARCH_VARIANT_NOT_SUPERSET_TAG", format!("{} fork-pin image_tag must end in -source-<fork track suffix>", r(&name)), format!("{pfx}.image_tag"));
            }
        } else if arch_suffix.is_empty() || architecture.as_deref() != Some(arch_suffix.as_str()) || !image_tag.ends_with(&format!("-source-{arch_suffix}")) {
            fail(&mut out, "ARCH_VARIANT_NOT_SUPERSET_TAG", format!("{} image_tag must end in -source-<track suffix>", r(&name)), format!("{pfx}.image_tag"));
        }
        let selectors = get(item, "build_patch_selectors").clone();
        let port_manifest = get(item, "port_manifest").clone();
        let has_selectors = !matches!(selectors.peel(), PyValue::None);
        let has_pm = !matches!(port_manifest.peel(), PyValue::None);
        let vref = field_or_empty("vllm_ref").unwrap_or_default();
        let ref_is_sha = full(r"[0-9a-f]{40}", &vref);
        if port_track {
            if str_of(get(item, "vllm_repo")).as_deref() != Some(STOCK_REPO) {
                fail(
                    &mut out,
                    "ARCH_VARIANT_PORT_TRACK_NOT_STOCK",
                    format!("{} is a self-port track and must pin the canonical upstream repo, not a fork", r(&name)),
                    format!("{pfx}.vllm_repo"),
                );
            }
            if !ref_is_sha && !full(r"v\d+\.\d+\.\d+(?:(?:rc|a|b|\.dev)\d+)?", &vref) {
                fail(
                    &mut out,
                    "ARCH_VARIANT_REF_NOT_SHA",
                    format!("{} vllm_ref must be a 40-hex commit SHA or an upstream release tag", r(&name)),
                    format!("{pfx}.vllm_ref"),
                );
            }
            if !has_selectors {
                fail(
                    &mut out,
                    "ARCH_VARIANT_PORT_SELECTORS_INVALID",
                    format!("{} is a self-port track and must declare the build_patch_selectors that create it", r(&name)),
                    format!("{pfx}.build_patch_selectors"),
                );
            }
            if !has_pm {
                fail(&mut out, "ARCH_VARIANT_PORT_MANIFEST_INVALID", format!("{} is a self-port track and requires a port_manifest", r(&name)), format!("{pfx}.port_manifest"));
            } else if !ref_is_sha && str_of(get(&port_manifest, "upstream_base_sha")).is_none() {
                fail(
                    &mut out,
                    "ARCH_VARIANT_PORT_MANIFEST_INVALID",
                    format!("{} pins a mutable tag and therefore requires port_manifest.upstream_base_sha", r(&name)),
                    format!("{pfx}.port_manifest.upstream_base_sha"),
                );
            }
        } else if fork_track {
            if str_of(get(item, "vllm_repo")).as_deref() == Some(STOCK_REPO) {
                fail(
                    &mut out,
                    "ARCH_VARIANT_FORK_TRACK_NOT_FORK",
                    format!("{} is a fork-pin track and must pin a fork repo, not the canonical upstream", r(&name)),
                    format!("{pfx}.vllm_repo"),
                );
            }
            if !ref_is_sha {
                fail(
                    &mut out,
                    "ARCH_VARIANT_REF_NOT_SHA",
                    format!("{} fork-pin vllm_ref must be a 40-hex commit SHA (forks can force-push)", r(&name)),
                    format!("{pfx}.vllm_ref"),
                );
            }
            if has_pm {
                fail(
                    &mut out,
                    "ARCH_VARIANT_PORT_MANIFEST_INVALID",
                    format!("{} is a fork-pin track and must not carry a port_manifest (the fork already ships the source)", r(&name)),
                    format!("{pfx}.port_manifest"),
                );
            }
        } else if !ref_is_sha {
            fail(&mut out, "ARCH_VARIANT_REF_NOT_SHA", format!("{} vllm_ref must be a 40-hex commit SHA", r(&name)), format!("{pfx}.vllm_ref"));
        }
        if has_selectors {
            validate_build_patch_selectors(&mut out, &name, &selectors, &pfx);
        }
        if has_pm {
            validate_port_manifest(&mut out, &name, &port_manifest, &pfx);
        }
        let grammar = r"easy-vllm:\d+\.\d+\.\d+-cu\d+-(?:aarch64|x86_64)-source-(?:sm\d+[a-z0-9]*|fork\d+)";
        if !full(grammar, &image_tag) {
            fail(
                &mut out,
                "ARCH_VARIANT_MODEL_KEYED_IMAGE",
                format!("{} image_tag must use the architecture-only canonical grammar (or the v3 fork-pin `-source-fork<PR>` form)", r(&name)),
                format!("{pfx}.image_tag"),
            );
        }
        all_tagged.push((name.clone(), image_tag.clone()));
        let status = field_or_empty("status");
        let superseded = status.as_deref().map_or(false, |st| starts(r"SUPERSEDED@\d+\.\d+\.\d+(?![\d.])", st));
        if status.as_deref().map_or(false, |st| st.starts_with("SUPERSEDED")) && !superseded {
            fail(
                &mut out,
                "ARCH_VARIANT_STATUS_INVALID",
                format!("{} superseded status must name the deprecating version as SUPERSEDED@<major.minor.patch> -- a bare marker must not hide an active candidate", r(&name)),
                format!("{pfx}.status"),
            );
        }
        let candidate = !superseded && status.as_deref().map_or(false, |st| starts(r"CANDIDATE\b", st));
        if !candidate {
            validate_bound_artifact(&mut out, &name, item, "evidence", "arch_variant_approval", &pfx);
        }
        if superseded {
            continue;
        }
        if candidate {
            candidates.push((name, item.clone()));
        } else {
            promoted.push((name, item.clone()));
        }
    }
    if promoted.len() > 1 {
        let names: Vec<&String> = promoted.iter().map(|(n, _)| n).collect();
        fail(
            &mut out,
            "ARCH_VARIANT_MULTIPLE_ACTIVE_TRACKS",
            format!("only one variant track may be active, got {}", list_repr(names)),
            format!("{LEDGER}.source_build_variants"),
        );
    }
    let mut tags_seen: Vec<(String, Vec<String>)> = Vec::new();
    for (name, tag) in &all_tagged {
        match tags_seen.iter_mut().find(|(t, _)| t == tag) {
            Some(slot) => slot.1.push(name.clone()),
            None => tags_seen.push((tag.clone(), vec![name.clone()])),
        }
    }
    tags_seen.sort_by(|a, b| a.0.cmp(&b.0));
    for (tag, mut names) in tags_seen {
        if names.len() > 1 {
            names.sort();
            fail(
                &mut out,
                "ARCH_VARIANT_IMAGE_TAG_COLLISION",
                format!(
                    "variants {} share image_tag {} -- distinct wiring must build to distinct images or one silently overwrites the other",
                    list_repr(&names),
                    r(&tag)
                ),
                format!("{LEDGER}.source_build_variants"),
            );
        }
    }
    for (name, item) in &candidates {
        let pfx = format!("{LEDGER}.source_build_variants.{name}");
        if has(item, "regression_evidence") {
            fail(
                &mut out,
                "ARCH_VARIANT_CANDIDATE_PREPROMOTED",
                format!("candidate variant {} must not carry regression evidence before promotion to VALIDATED", r(name)),
                format!("{pfx}.regression_evidence"),
            );
        }
        validate_candidate_evidence(&mut out, name, item, &pfx);
    }
    for (name, item) in &promoted {
        let pfx = format!("{LEDGER}.source_build_variants.{name}");
        if str_of(get(item, "status")).as_deref() != Some("VALIDATED") {
            fail(&mut out, "ARCH_VARIANT_ACTIVE_NOT_VALIDATED", format!("active variant {} lacks VALIDATED status", r(name)), format!("{pfx}.status"));
        }
        if !is_dict(get(item, "regression_evidence")) {
            fail(
                &mut out,
                "ARCH_VARIANT_REGRESSION_EVIDENCE_MISSING",
                format!("active variant {} requires existing-model regression re-smoke evidence", r(name)),
                format!("{pfx}.regression_evidence"),
            );
        } else {
            validate_bound_artifact(&mut out, name, item, "regression_evidence", "arch_variant_regression", &pfx);
        }
    }
    let required_docker = ["ARG VLLM_REPO=", "ARG VLLM_REF=", "--filter=blob:none", "checkout --detach ${VLLM_REF}"];
    if !dockerfile.is_empty() && !required_docker.iter().all(|t| dockerfile.contains(t)) {
        fail(&mut out, "ARCH_VARIANT_SOURCE_OVERRIDE_UNWIRED", "source template lacks pinned repo/ref checkout wiring".into(), dockerfile_rel.into());
    }
    let workflow = read_text(&repo_root.join(".claude/rules/workflow.md"));
    let skill = read_text(&repo_root.join(".claude/skills/upstream-version-watch/SKILL.md"));
    let (workflow, skill) = match (workflow, skill) {
        (Ok(w), Ok(sk)) => (w, sk),
        (Err(e), _) | (_, Err(e)) => {
            fail(&mut out, "ARCH_VARIANT_PROCEDURE_UNREADABLE", e, ".claude/rules/workflow.md".into());
            return out;
        }
    };
    let ladder = "deps-패치 → 소스-게이트 패치 → **자체 이식** → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체";
    if !workflow.contains(ladder) {
        fail(&mut out, "ARCH_VARIANT_LADDER_ORDER_MISSING", "fixed no-skip ladder is missing".into(), ".claude/rules/workflow.md".into());
    }
    let ordered = ["참조-그라운디드 확증", "testlog 기록 + 사람 승인", "source_build_variants", "Dockerfile build-arg 파라미터화", "clean 빌드(양노드) → 스모크"];
    let order_ok = match workflow.find("stock-구조적-불가") {
        None => false,
        Some(anchor) => {
            let mut positions = Vec::new();
            let mut ok = true;
            for tok in ordered {
                match workflow[anchor..].find(tok) {
                    Some(p) => positions.push(anchor + p),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            ok && positions.windows(2).all(|w| w[0] <= w[1])
        }
    };
    if !order_ok {
        fail(
            &mut out,
            "ARCH_VARIANT_HITL_EVIDENCE_ORDER_INVALID",
            "reference evidence -> testlog/HITL -> ledger -> build args -> clean smoke order is required".into(),
            ".claude/rules/workflow.md".into(),
        );
    }
    if !skill.contains("기존모델 회귀 재스모크") || !skill.contains("단일 변종-트랙") || !skill.contains("무증거 오버라이드 금지") {
        fail(
            &mut out,
            "ARCH_VARIANT_PROMOTION_GUARDS_MISSING",
            "procedure must require existing-model regression re-smoke, one active track, and evidence".into(),
            ".claude/skills/upstream-version-watch/SKILL.md".into(),
        );
    }
    out
}
