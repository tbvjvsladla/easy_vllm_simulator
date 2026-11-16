//! `companion_predicate.py` (unittest): the companion structural checks of HOST_SAFETY_LAYERED_DEFENSE.C3,
//! LAST_GOOD_ROLLBACK_ANCHOR.C1-C3, MODEL_ACQUISITION_TERNARY_GATE.C1/C2/C4 and TERRAFORM_FLAG_GATE.C4.
use crate::pyast;
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

const TAG_PY: &str = ".claude/skills/hint-publisher/scripts/hintlib/tag.py";

fn test_hint_push_sends_exactly_one_tag_refspec() -> R {
    let src = read(TAG_PY)?;
    let logicals = pyast::tokenize(&src);
    // tree.body FunctionDef only (top level, not async)
    let fns: Vec<pyast::FuncDef> = pyast::functions(&logicals).into_iter().filter(|f| f.indent == 0 && !f.is_async).collect();
    let get = |n: &str| fns.iter().rev().find(|f| f.name == n).cloned();
    for name in ["push_tag", "_tag_ref", "check_ref_format", "git_push_authenticated"] {
        req(get(name).is_some(), format!("{} not found in {} : hintlib.tag.{name} must exist", repr(name), "fns"))?;
    }
    let push_src = pyast::segment(&src, &get("push_tag").unwrap());
    assert_in("check_ref_format(repo, tag)", &push_src, None)?;
    assert_in("refspec = f\"{ref}:{ref}\"", &push_src, None)?;
    assert_in("return f\"refs/tags/{tag}\"", &pyast::segment(&src, &get("_tag_ref").unwrap()), None)?;
    let guard_src = pyast::segment(&src, &get("check_ref_format").unwrap());
    assert_in("not tag.startswith(core.HINT_TAG_PREFIX)", &guard_src, None)?;
    assert_in("_GLOB_CHARS", &guard_src, None)?;
    assert_in("HINT_TAG_PREFIX = \"hint/\"", &read(".claude/skills/hint-publisher/scripts/hintlib/core.py")?, None)?;
    assert_not_in("\"refs/tags/hint/*\"", &push_src, None)?;
    for name in ["push_tag", "git_push_authenticated"] {
        let literals = pyast::function_str_constants(&logicals, &get(name).unwrap());
        req(!literals.iter().any(|l| l == "--tags"), format!("'--tags' unexpectedly found in literals : {name} must never pass --tags"))?;
    }
    Ok(())
}

fn test_retired_glob_push_cli_is_gone() -> R {
    let scripts = repo_root().join(".claude/skills/hint-publisher/scripts");
    let mut top: Vec<String> = std::fs::read_dir(&scripts).map_err(|e| Fail::new("FileNotFoundError", e.to_string()))?
        .flatten().map(|e| e.path()).filter(|p| p.is_file() && matches!(p.extension().and_then(|x| x.to_str()), Some("py") | Some("sh")))
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    top.sort();
    req(top == ["hint.py", "render_bench_section.py"],
        format!("Lists differ: {} != ['hint.py', 'render_bench_section.py'] : hint-publisher/scripts must hold only the single CLI and the bench-section renderer (+ hintlib/) -- a retired CLI must be removed, not kept as a shim", repr_list(&top)))?;
    req(scripts.join("hintlib/tag.py").is_file(), "False is not true : the push path lives in hintlib/tag.py")
}

fn test_sync_branches_never_tags_or_hard_resets() -> R {
    let src = read(".claude/skills/upstream-version-watch/scripts/sync_branches.sh")?;
    assert_not_in("git tag", &src, None)?;
    assert_not_in("git reset --hard", &src, None)
}

fn test_manifest_contract_enforces_ternary_model_source() -> R {
    let literal_tuple = |path: &str| -> R<Option<Vec<String>>> {
        let logicals = pyast::tokenize(&read(path)?);
        match pyast::top_level_tuple_of_str(&logicals, "VALID_MODES") {
            None => Ok(None),
            Some(None) => Err(Fail::new("AssertionError", format!("value is not an instance of Tuple : {path}"))),
            Some(Some(v)) => Ok(Some(v)),
        }
    };
    let scripts = ".claude/skills/terraforming_node/scripts/";
    let adapter = literal_tuple(&format!("{scripts}manifest_contract.py"))?;
    let compat = literal_tuple(&format!("{scripts}manifest_contract_compat_owner.py"))?;
    let gate = read(".claude/rust/role-contract/src/manifest_gate.rs")?;
    let rust = re("(?m)^pub const VALID_MODES: \\[&str; (\\d+)\\] = \\[([^\\]]*)\\];").captures(&gate).ok().flatten();
    req(rust.is_some(), "unexpectedly None : manifest_gate.rs must declare `pub const VALID_MODES: [&str; N] = [...]`")?;
    let rust = rust.unwrap();
    let rust_modes = findall("\"([^\"]*)\"", rust.get(2).unwrap().as_str());
    let n: usize = rust.get(1).unwrap().as_str().parse().unwrap_or(usize::MAX);
    req(n == rust_modes.len(), format!("{n} != {}", rust_modes.len()))?;
    req(rust_modes == ["managed", "ephemeral", "custom"], format!("Tuples differ: {} != ('managed', 'ephemeral', 'custom')", repr_list(&rust_modes)))?;
    req(adapter.as_ref() == Some(&rust_modes), format!("Tuples differ: adapter {:?} != {:?}", adapter, rust_modes))?;
    req(compat.as_ref() == Some(&rust_modes), format!("Tuples differ: compat {:?} != {:?}", compat, rust_modes))
}

fn test_check_smoke_model_has_no_download_machinery() -> R {
    let src = read(".claude/skills/upstream-version-watch/scripts/check_smoke_model.py")?;
    for banned in ["urllib.request", "requests.", "snapshot_download", "huggingface_hub", "subprocess.run(['wget'", "subprocess.run([\"wget\""] {
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
