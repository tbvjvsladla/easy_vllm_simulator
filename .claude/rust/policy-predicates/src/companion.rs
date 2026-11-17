//! `companion_predicate.py` (unittest): the companion structural checks of HOST_SAFETY_LAYERED_DEFENSE.C3,
//! LAST_GOOD_ROLLBACK_ANCHOR.C1-C3, MODEL_ACQUISITION_TERNARY_GATE.C1/C2/C4 and TERRAFORM_FLAG_GATE.C4.
use crate::support::*;
use crate::unit::Case;

fn assert_in(needle: &str, hay: &str, msg: Option<&str>) -> R {
    req(hay.contains(needle), match msg {
        Some(m) => format!("{} not found in {} : {m}", repr(needle), repr(hay)),
        None => format!("{} not found in {}", repr(needle), repr(hay)),
    })
}

fn assert_not_in(needle: &str, hay: &str, msg: Option<&str>) -> R {
    req(!hay.contains(needle), match msg {
        Some(m) => format!("{} unexpectedly found in {} : {m}", repr(needle), repr(hay)),
        None => format!("{} unexpectedly found in {}", repr(needle), repr(hay)),
    })
}

fn test_scan_node_documents_host_safety_flag_independence() -> R {
    let src = read(".claude/rust/role-contract/src/scan_node_manifest.rs")?;
    assert_in("host_safety.installed", &src, None)?;
    assert_in("Flag 와 독립", &src, None)
}

/// The push path of hint-publisher is the Rust `hintlib::tag` module (plan_26111615 D1 -- `hintlib/tag.py` is a
/// reference original pending deletion and is no longer the evidence).
const TAG_RS: &str = ".claude/skills/hint-publisher/rust/src/tag.rs";
const CORE_RS: &str = ".claude/skills/hint-publisher/rust/src/core.rs";

fn test_hint_push_sends_exactly_one_tag_refspec() -> R {
    let src = read(TAG_RS)?;
    for name in ["push_tag", "_tag_ref", "check_ref_format", "git_push_authenticated"] {
        req(rust_top_fn(&src, name).is_some(), format!("{} not found in {} : hintlib::tag::{name} must exist", repr(name), "fns"))?;
    }
    let push_src = rust_top_fn(&src, "push_tag").unwrap();
    assert_in("check_ref_format(repo, tag)?", &push_src, None)?;
    assert_in("let r = tref(tag);", &push_src, None)?;
    assert_in("let refspec = format!(\"{r}:{r}\");", &push_src, None)?;
    assert_in("Ok(tref(tag))", &rust_top_fn(&src, "_tag_ref").unwrap(), None)?;
    let tref_src = rust_top_fn(&src, "tref");
    req(tref_src.is_some(), "'tref' not found in fns : hintlib::tag::tref must exist")?;
    assert_in("format!(\"refs/tags/{tag}\")", &tref_src.unwrap(), None)?;
    let guard_src = rust_top_fn(&src, "check_ref_format").unwrap();
    assert_in("!tag.starts_with(core::HINT_TAG_PREFIX)", &guard_src, None)?;
    assert_in("_GLOB_CHARS", &guard_src, None)?;
    assert_in("pub const HINT_TAG_PREFIX: &str = \"hint/\";", &read(CORE_RS)?, None)?;
    assert_not_in("\"refs/tags/hint/*\"", &push_src, None)?;
    let scope = search("(?m)^pub const PUSH_SCOPE_ARGS: [^\\n]*$", &src).map(|m| m.2).unwrap_or_default();
    req(!scope.is_empty(), "unexpectedly None : hintlib::tag::PUSH_SCOPE_ARGS must exist")?;
    for (name, body) in [("push_tag", push_src.clone()), ("git_push_authenticated", rust_top_fn(&src, "git_push_authenticated").unwrap()), ("PUSH_SCOPE_ARGS", scope)] {
        req(!body.contains("\"--tags\""), format!("'--tags' unexpectedly found in literals : {name} must never pass --tags"))?;
    }
    Ok(())
}

fn test_retired_glob_push_cli_is_gone() -> R {
    let scripts = repo_root().join(".claude/skills/hint-publisher/scripts");
    let mut top: Vec<String> = std::fs::read_dir(&scripts).map_err(|e| Fail::new("FileNotFoundError", e.to_string()))?
        .flatten().map(|e| e.path()).filter(|p| p.is_file() && matches!(p.extension().and_then(|x| x.to_str()), Some("py") | Some("sh")))
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    top.sort();
    // The programs are the Rust launchers (closed list = tripwire). The two `.py` originals may still sit beside them
    // until their physical deletion (plan_26111615 P6) but nothing else may: any other `.py` is a retired CLI.
    const LAUNCHERS: [&str; 4] = ["build_rust.sh", "hint.sh", "hint_api.sh", "render_bench_section.sh"];
    const ORIGINALS: [&str; 2] = ["hint.py", "render_bench_section.py"];
    let launchers: Vec<&String> = top.iter().filter(|n| n.ends_with(".sh")).collect();
    let stray: Vec<&String> = top.iter().filter(|n| n.ends_with(".py") && !ORIGINALS.contains(&n.as_str())).collect();
    req(launchers.iter().map(|s| s.as_str()).eq(LAUNCHERS.iter().copied()) && stray.is_empty(),
        format!("Lists differ: {} != {} (+ at most {}) : hint-publisher/scripts must hold only the single CLI and the bench-section renderer launchers (+ the build/api launchers) -- a retired CLI must be removed, not kept as a shim", repr_list(&top), repr_list(&LAUNCHERS), repr_list(&ORIGINALS)))?;
    req(repo_root().join(TAG_RS).is_file(), format!("False is not true : the push path lives in {TAG_RS}"))
}

fn test_sync_branches_never_tags_or_hard_resets() -> R {
    let src = read(".claude/skills/upstream-version-watch/scripts/sync_branches.sh")?;
    assert_not_in("git tag", &src, None)?;
    assert_not_in("git reset --hard", &src, None)
}

fn test_manifest_contract_enforces_ternary_model_source() -> R {
    // The single enforcing owner of the three modes is the Rust gate (`manifest_gate.rs` VALID_MODES, the const
    // `mode_valid` reads). The Python adapter/compat tuples it used to be cross-checked with are reference originals
    // pending deletion (plan_26111615 P6) -- no longer an authority, so no longer read.
    let gate = read(".claude/rust/role-contract/src/manifest_gate.rs")?;
    req(gate.contains("VALID_MODES.iter().any(|mode| text.equals(mode))"),
        "'VALID_MODES.iter().any(|mode| text.equals(mode))' not found : the mode check must read VALID_MODES (no second list)")?;
    let rust = re("(?m)^pub const VALID_MODES: \\[&str; (\\d+)\\] = \\[([^\\]]*)\\];").captures(&gate).ok().flatten();
    req(rust.is_some(), "unexpectedly None : manifest_gate.rs must declare `pub const VALID_MODES: [&str; N] = [...]`")?;
    let rust = rust.unwrap();
    let rust_modes = findall("\"([^\"]*)\"", rust.get(2).unwrap().as_str());
    let n: usize = rust.get(1).unwrap().as_str().parse().unwrap_or(usize::MAX);
    req(n == rust_modes.len(), format!("{n} != {}", rust_modes.len()))?;
    req(rust_modes == ["managed", "ephemeral", "custom"], format!("Tuples differ: {} != ('managed', 'ephemeral', 'custom')", repr_list(&rust_modes)))
}

/// `check_smoke_model` is the upstream runtime's Rust program (`smoke_model.rs`, started by `check_smoke_model.sh`).
fn test_check_smoke_model_has_no_download_machinery() -> R {
    let launcher = read(".claude/skills/upstream-version-watch/scripts/check_smoke_model.sh")?;
    assert_in("exec -a \"$SDIR/check_smoke_model.sh\" \"$binary\"", &launcher, Some("check_smoke_model.sh must start the native program"))?;
    let src = read(".claude/skills/upstream-version-watch/rust/runtime/src/smoke_model.rs")?;
    assert_in("pub const PROGRAM: &str = \"check_smoke_model.sh\";", &src, Some("smoke_model.rs must be the check_smoke_model program"))?;
    for banned in ["ureq", "reqwest", "hf_http", "snapshot_download", "huggingface_hub", "Command::new(\"wget\")", "Command::new(\"curl\")", "\"wget\"", "\"curl\"", "\"git-lfs\"", "\"lfs\""] {
        assert_not_in(banned, &src, None)?;
    }
    Ok(())
}

fn test_manifest_template_hf_token_field_is_a_pointer_not_a_raw_value() -> R {
    let src = read("manifest.template.yaml")?;
    assert_in("hf_token_env_file:", &src, None)?;
    req(search("hf_token_env_file:\\s*\"\"", &src).is_some(), "Regex didn't match: 'hf_token_env_file:\\\\s*\"\"' not found")
}

fn test_wiki_desk_scripts_never_reference_the_flag_gate() -> R {
    let wiki = repo_root().join(".claude/skills/wiki-desk");
    let mut files = Vec::new();
    for sub in ["src", "scripts"] {
        if let Ok(rd) = std::fs::read_dir(wiki.join(sub)) {
            for e in rd.flatten() {
                let p = e.path();
                if p.file_name().map_or(false, |n| n.to_string_lossy().ends_with(".ts")) {
                    files.push(p);
                }
            }
        }
    }
    files.sort();
    req(!files.is_empty(), "[] is not true : expected at least one wiki-desk script to scan")?;
    let py: Vec<String> = rglob_files(&wiki).into_iter().filter(|p| p.ends_with(".py")).collect();
    req(py.is_empty(), format!("Lists differ: {} != [] : wiki-desk 안에 Python 이 남았다(D6 · TS 전환)", repr_list(&py)))?;
    let banned = ["manifest_contract", "_require_terraform_flag", "terraform.complete", "a2a_delegation"];
    let mut offenders = Vec::new();
    for f in &files {
        let text = read_path(f)?;
        for token in banned {
            if text.contains(token) {
                offenders.push(format!("('{}', '{token}')", f.file_name().unwrap().to_string_lossy()));
            }
        }
    }
    req(offenders.is_empty(), format!("Lists differ: [{}] != []", offenders.join(", ")))
}

pub fn cases() -> Vec<Case> {
    vec![
        Case { module: "companion_predicate", class: "TestHostSafetyLayeredDefenseCompanion", name: "test_scan_node_documents_host_safety_flag_independence", f: test_scan_node_documents_host_safety_flag_independence },
        Case { module: "companion_predicate", class: "TestLastGoodRollbackAnchorCompanion", name: "test_hint_push_sends_exactly_one_tag_refspec", f: test_hint_push_sends_exactly_one_tag_refspec },
        Case { module: "companion_predicate", class: "TestLastGoodRollbackAnchorCompanion", name: "test_retired_glob_push_cli_is_gone", f: test_retired_glob_push_cli_is_gone },
        Case { module: "companion_predicate", class: "TestLastGoodRollbackAnchorCompanion", name: "test_sync_branches_never_tags_or_hard_resets", f: test_sync_branches_never_tags_or_hard_resets },
        Case { module: "companion_predicate", class: "TestModelAcquisitionTernaryGateCompanion", name: "test_check_smoke_model_has_no_download_machinery", f: test_check_smoke_model_has_no_download_machinery },
        Case { module: "companion_predicate", class: "TestModelAcquisitionTernaryGateCompanion", name: "test_manifest_contract_enforces_ternary_model_source", f: test_manifest_contract_enforces_ternary_model_source },
        Case { module: "companion_predicate", class: "TestModelAcquisitionTernaryGateCompanion", name: "test_manifest_template_hf_token_field_is_a_pointer_not_a_raw_value", f: test_manifest_template_hf_token_field_is_a_pointer_not_a_raw_value },
        Case { module: "companion_predicate", class: "TestTerraformFlagGateCompanion", name: "test_wiki_desk_scripts_never_reference_the_flag_gate", f: test_wiki_desk_scripts_never_reference_the_flag_gate },
    ]
}
