//! Ported clause predicates whose original calls another bundle's Python module in-process, retargeted to that
//! module's Rust owner: scan_node / manifest_contract (`role_contract` library), render_dockerfile and slave_forward
//! (their Rust command lines behind the launchers). Same checks, same order, same messages.
use crate::support::*;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn predicate_MODEL_ACQUISITION_TERNARY_GATE_C1() -> R {
    // The single owner of the three modes is the Rust gate (`role_contract::manifest_gate::VALID_MODES`, the const the
    // mode check reads). The Python adapter / compat-owner tuples it used to be compared with are reference originals
    // pending deletion (plan_26111615 P6) and are no longer evidence.
    req(role_contract::manifest_gate::VALID_MODES == ["managed", "ephemeral", "custom"], "predicate requirement failed at original line 1069")?;
    let gate = read(".claude/rust/role-contract/src/manifest_gate.rs")?;
    let rust_modes = re("(?m)^pub const VALID_MODES: \\[&str; (\\d+)\\] = \\[([^\\]]*)\\];").captures(&gate).ok().flatten();
    req(rust_modes.is_some(), "manifest_gate.rs must declare VALID_MODES")?;
    req(findall("\"([^\"]*)\"", rust_modes.unwrap().get(2).unwrap().as_str()) == role_contract::manifest_gate::VALID_MODES, "Rust VALID_MODES source differs from the compiled tuple")?;
    req(gate.contains("VALID_MODES.iter().any(|mode| text.equals(mode))"), "the mode check must read VALID_MODES (no second mode list)")?;
    let ems = |node: &str, top: &str| -> R<Option<String>> {
        Ok(jstr(&role_contract::manifest_gate::effective_model_source(&json_loads(node)?, &json_loads(top)?)))
    };
    req(ems("{\"model_source\": \"ephemeral\"}", "\"managed\"")?.as_deref() == Some("ephemeral"), "predicate requirement failed at original line 1070")?;
    req(ems("{}", "\"managed\"")?.as_deref() == Some("managed"), "predicate requirement failed at original line 1071")?;
    req(ems("null", "\"custom\"")?.as_deref() == Some("custom"), "predicate requirement failed at original line 1072")?;
    let res = evaluate("{\"topology\": \"single\", \"gpus_per_node\": 1, \"model_source\": \"custom\", \"nas_model_path\": \"/mnt/models\", \"custom_model_paths\": {\"my-model\": \"/data/models/my-model\"}, \"nodes\": [], \"terraforming\": {\"complete\": true, \"branch_verified\": true}}", "single")?;
    req(matches!(res.get("flag").map(PyValue::peel), Some(PyValue::Bool(true))), "predicate requirement failed at original line 1083")?;
    req(res.get("nas_model_path").and_then(jstr).as_deref() == Some("/mnt/models"), "predicate requirement failed at original line 1084")?;
    let cmp = match res.get("custom_model_paths").map(PyValue::peel) {
        Some(PyValue::Dict(d)) => d.len() == 1 && d.get("my-model").and_then(jstr).as_deref() == Some("/data/models/my-model"),
        _ => false,
    };
    req(cmp, "predicate requirement failed at original line 1085")?;
    let compose = rendered("compose")?;
    req(search("NAS_MODEL_PATH.*:/app/models:ro", &compose).is_some(), "managed/custom model mount must be read-only")?;
    req(compose.contains("(managed/custom 마운트원)"), "the read-only host mount must be documented as the managed/custom storage location")?;
    req(search("(?m)^volumes:\\s*$", &compose).is_none(), "no top-level Docker-managed named volume may back model storage -- ephemeral's container-internal cache must be deletable by container removal alone, never persisted by a volume")?;
    let tmpl = read("manifest.template.yaml")?;
    req(tmpl.contains("컨테이너 내부 HF 캐시에 임시 다운로드(컨테이너 down→삭제)"), "ephemeral must be documented as container-cache download, deleted on container down")?;
    req(tmpl.contains("사용자 지정 경로에 모델 저장·볼륨마운트") && tmpl.contains("custom_model_paths 매핑 사용"), "custom must be documented as a user-specified path, volume-mounted")?;
    req(tmpl.contains("관리 NAS read-only 마운트 기본·권장"), "managed must be documented as a read-only NAS mount")?;
    req(tmpl.contains("서빙 런타임 마운트는 read-only"), "the cross-mode invariant (runtime mount is always read-only) must be documented")
}

/// `manifest_contract.main(argv)` = the Flag-gate command line (`manifest_contract.sh`, Rust `manifest_contract`): its exit code.
fn manifest_contract_main(args: &[&str]) -> R<i32> {
    let l = repo_root().join(".claude/skills/terraforming_node/scripts/manifest_contract.sh").to_string_lossy().into_owned();
    let mut argv = vec!["bash", l.as_str()];
    argv.extend_from_slice(args);
    Ok(run(&argv, None, &[], None, Some(120))?.code)
}

pub fn predicate_TERRAFORM_FLAG_GATE_C4() -> R {
    let wiki_dir = repo_root().join(".claude/skills/wiki-desk");
    let mut ts_files = Vec::new();
    for sub in ["src", "scripts"] {
        if let Ok(rd) = std::fs::read_dir(wiki_dir.join(sub)) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().ends_with(".ts") {
                    ts_files.push(e.path());
                }
            }
        }
    }
    ts_files.sort();
    req(!ts_files.is_empty(), "wiki-desk TS sources must exist to be scanned")?;
    for f in &ts_files {
        let text = read_path(f)?;
        for token in ["manifest_contract", "_require_terraform_flag", "terraforming.complete", "a2a_delegation"] {
            req(!text.contains(token), format!("{} must never reference {}", f.file_name().unwrap().to_string_lossy(), repr(token)))?;
        }
    }
    let cli = wiki_dir.join("src/cli.ts").to_string_lossy().into_owned();
    let mut contract = json_loads(&read_path(&wiki_dir.join("assets/project-contract.example.json"))?)?;
    if let PyValue::Dict(d) = &mut contract {
        d.set("source_roots", PyValue::List(vec![PyValue::str("docs")]));
        d.set("excluded_roots", PyValue::List(Vec::new()));
    }
    let contract_text = contract.dumps_ascii().map_err(|e| Fail::new("TypeError", format!("{e:?}")))?;
    let run_query = |repo: &Path| -> R<(i32, String)> {
        write(&repo.join("docs/devlog/devlog_26072508_x.md"), "# flag gate probe\n\nflag gate probe body\n")?;
        write(&repo.join("contract.json"), &contract_text)?;
        let (c, r) = (repo.join("contract.json").to_string_lossy().into_owned(), repo.to_string_lossy().into_owned());
        let steps: [Vec<&str>; 3] = [vec!["init", "--contract", c.as_str(), "--apply"], vec!["sync", "--apply", "--as", "all"], vec!["query", "--terms", "flag gate probe"]];
        let mut last = String::new();
        for step in steps {
            let mut argv = vec!["node", cli.as_str()];
            argv.extend(step);
            argv.extend(["--root", r.as_str(), "--local"]);
            let out = run(&argv, None, &[], None, None)?;
            if out.code != 0 {
                return Ok((out.code, out.stderr));
            }
            last = out.stdout;
        }
        Ok((0, last))
    };
    let tmp = TempDir::new("tmp")?;
    let absent = tmp.path().join("absent");
    mkdirs(&absent)?;
    let mc_absent = manifest_contract_main(&["--topology", "single", "--repo", &absent.to_string_lossy(), "--json"])?;
    req(mc_absent == role_contract::manifest_gate::EXIT_NO_MANIFEST, "control: this repo state must genuinely be Flag-absent")?;
    let (rc_absent, out_absent) = run_query(&absent)?;
    let present = tmp.path().join("present");
    write(&present.join("output/single/manifest.yaml"), "topology: single\ngpus_per_node: 1\nmodel_source: managed\nnodes: []\nterraforming:\n  complete: true\n  branch_verified: true\n")?;
    let mc_present = manifest_contract_main(&["--topology", "single", "--repo", &present.to_string_lossy(), "--require-flag"])?;
    req(mc_present == role_contract::manifest_gate::EXIT_OK, "control: this repo state must genuinely be Flag-valid")?;
    let (rc_present, out_present) = run_query(&present)?;
    req(rc_absent == 0 && rc_present == 0, format!("wiki-desk's real CLI must succeed identically regardless of Flag state, got exit {rc_absent} (Flag-absent) vs {rc_present} (Flag-present): {}", if rc_absent != 0 { &out_absent } else { &out_present }))?;
    let hits = |o: &str| -> R<Vec<String>> {
        Ok(jlist(jget(&json_loads(o)?, "results")?).iter().filter_map(|r| jget(r, "path").ok().and_then(jstr)).collect())
    };
    let (a, b) = (hits(&out_absent)?, hits(&out_present)?);
    req(a == b && a == ["docs/devlog/devlog_26072508_x.md"], format!("both runs must find the same probe document, not merely exit 0: [{}, {}]", repr_list(&a), repr_list(&b)))
}

pub fn predicate_MODEL_ACQUISITION_TERNARY_GATE_C2() -> R {
    // check_smoke_model is the upstream runtime's Rust program (`smoke_model.rs` behind `check_smoke_model.sh`); the
    // crosscheck CLI is the recipe runtime's `model_cli.rs` (plan_26111615: the `.py` modules are no longer evidence).
    let launcher = read(".claude/skills/upstream-version-watch/scripts/check_smoke_model.sh")?;
    req(launcher.contains("exec -a \"$SDIR/check_smoke_model.sh\" \"$binary\""), "check_smoke_model.sh must start the native smoke-model program")?;
    let src = read(".claude/skills/upstream-version-watch/rust/runtime/src/smoke_model.rs")?;
    req(src.contains("pub const PROGRAM: &str = \"check_smoke_model.sh\";"), "smoke_model.rs must be the check_smoke_model program")?;
    for banned in ["ureq", "reqwest", "hf_http", "urllib", "requests", "snapshot_download", "huggingface_hub", "wget", "curl", "TcpStream", "UdpSocket", "std::net", "socket", "libc::system", "popen", "Popen"] {
        req(!src.contains(banned), format!("no download/shell mechanism {} may appear in check_smoke_model (smoke_model.rs)", repr(banned)))?;
    }
    req(count(&src, "Command::new(") == 2, "the only subprocess calls must be the local rope translator and the local spec-layout launcher, never a download command")?;
    let rope_fn = rust_top_fn(&src, "rope_check").unwrap_or_default();
    req(rope_fn.contains("vllm-recipe-explorer/rust/runtime/target/release/rope_scaling_translate") && rope_fn.contains("Command::new(&elf)"),
        "one subprocess call must run the local rope translator program")?;
    req(src.contains("Command::new(\"bash\")\n                    .arg(&spec_script)") && src.contains("scripts.join(\"check_spec_layout.sh\")"),
        "one subprocess call must run bash on the local check_spec_layout.sh launcher, never a download command")?;
    req(!src.contains("model_source"), "the serve-time model-presence gate must never branch on model_source -- selecting a mode (even complete+ephemeral) must not, by itself, be able to authorize anything at this gate")?;
    let stop_idx = index(&src, "\"[NAS-check] STOP: 모델 부재")?;
    let exit_idx = index_from(&src, "\n    2\n}", stop_idx)?;
    req(src[stop_idx..exit_idx].contains("다운로드하지 않음"), "the exit(2) STOP path must literally state the no-download rule")?;
    let ccsrc = read(".claude/skills/vllm-recipe-explorer/rust/runtime/src/model_cli.rs")?;
    let spec = rust_top_fn(&ccsrc, "crosscheck_spec").unwrap_or_default();
    req(search("opt\\(\\s*&\\[\"--hf-repo-id\"\\],\\s*\"hf_repo_id\",\\s*Kind::Text,", &spec).is_some() && !spec.contains("default"),
        "--hf-repo-id must have no default -- each fetch names its target explicitly, per-event")?;
    for flag in ["--yes", "--auto-approve", "--no-confirm", "--skip-confirm", "--force"] {
        req(!spec.contains(flag), format!("no blanket-authorization flag ({flag}) may exist on the external-fetch CLI"))?;
    }
    // inspect.getsource(crosscheck_model_card.main) -> the Rust `crosscheck_main`
    let main_src = rust_top_fn(&ccsrc, "crosscheck_main")
        .ok_or_else(|| Fail::new("AttributeError", "module 'crosscheck_model_card' has no attribute 'main'"))?;
    req(main_src.contains("--hf-repo-id 필수"), "missing --hf-repo-id on --ephemeral-estimate must abort rather than silently proceeding with an implicit/blanket target")?;
    let missing_idx = index(&main_src, "--hf-repo-id 필수")?;
    let exit3_idx = index_from(&main_src, "streams.finish(3)", missing_idx)?;
    req(exit3_idx > missing_idx, "the missing-repo-id message must precede a real abort(exit 3)")
}

struct FixedClock;
impl role_contract::scan_node_manifest::Clock for FixedClock {
    fn local_hour(&self) -> agent_runtime_support::pystr::PyStr {
        agent_runtime_support::pystr::PyStr::new("2026-01-01T00")
    }
}

/// `manifest_contract.evaluate_contract(man, topology)` (Rust owner `role_contract::manifest_gate`).
pub fn evaluate(man: &str, topology: &str) -> R<PyDict> {
    let man = json_loads(man)?;
    role_contract::manifest_gate::evaluate_contract(&man, topology).map(|e| e.res).map_err(|e| Fail::new("RuntimeError", format!("{e:?}")))
}

pub fn predicate_HOST_SAFETY_LAYERED_DEFENSE_C3() -> R {
    let scan_src = read(".claude/rust/role-contract/src/scan_node_manifest.rs")?;
    let result = json_loads("{\"topology_declared\": \"single\", \"cpu_arch\": \"aarch64\", \"cuda_version\": \"132\", \"gpus_per_node\": 1, \"interconnect\": {\"type\": \"generic-ethernet\", \"hca_devices\": [], \"gid_index\": null, \"socket_iface\": null, \"bandwidth_gbps\": null, \"platform_preset\": null}}")?;
    let block = role_contract::scan_node_manifest::emit_manifest_block(&result, &FixedClock).map_err(|e| Fail::new("RuntimeError", format!("{e:?}")))?;
    let block = jstr(&block).unwrap_or_default();
    req(block.contains("terraforming:") && block.contains("complete: true"), "predicate requirement failed at original line 282")?;
    req(!block.contains("host_safety:"), "predicate requirement failed at original line 283")?;
    req(scan_src.contains("host_safety.installed") && scan_src.contains("Flag 와 독립"), "predicate requirement failed at original line 284")?;
    let base = "\"topology\": \"single\", \"gpus_per_node\": 1, \"model_source\": \"managed\", \"terraforming\": {\"complete\": true, \"branch_verified\": true}";
    let r_installed = evaluate(&format!("{{{base}, \"host_safety\": {{\"installed\": true}}}}"), "single")?;
    let r_optout = evaluate(&format!("{{{base}, \"host_safety\": {{\"installed\": false}}}}"), "single")?;
    let flag = |d: &PyDict| matches!(d.get("flag").map(PyValue::peel), Some(PyValue::Bool(true)));
    req(flag(&r_installed) && flag(&r_optout), "the completion Flag must stay valid whether or not host_safety is installed")?;
    req(!r_installed.contains_key("host_safety") && !r_optout.contains_key("host_safety"), "evaluate_contract must never echo/branch on host_safety at all -- true independence")?;
    let tmpl = read("manifest.template.yaml")?;
    req(search("(?m)^host_safety:", &tmpl).is_some(), "manifest must carry a host_safety field")?;
    req(tmpl.contains("통합메모리 노드면 이후 서빙 기동 시 에이전트 채팅 1줄 경고"), "opt-out unified-memory follow-up must be documented as exactly one chat-only line")?;
    let skill_src = read(".claude/skills/terraforming_node/SKILL.md")?;
    req(skill_src.contains("discrete GPU 노드는 무경고"), "the warning must be scoped to unified memory only")?;
    req(skill_src.contains("serve 스크립트/로그 배너 코드변경 ✗"), "must be documented as explicitly excluding any serve-script/log banner code change")?;
    let banned_phrase = "워치독 미설치 상태";
    for text in [shared_asset("serve_runner.sh")?, shared_asset("debug-init.sh")?, shared_asset("arm_patch.sh")?, rendered("compose")?] {
        req(!text.contains(banned_phrase), "shared runtime sources must never carry a serve-script banner for this warning")?;
    }
    Ok(())
}

/// `render_dockerfile.build_cluster_env(manifest)` through `render_dockerfile.sh --cluster-envfile` (the envfile is
/// rendered from exactly that dict: one `KEY=VALUE` line per entry).
fn cluster_envfile(manifest_yaml: &str) -> R<String> {
    let tmp = TempDir::new("policy-predicates-cluster-")?;
    let m = tmp.path().join("manifest.yaml");
    write(&m, manifest_yaml)?;
    let l = repo_root().join(".claude/skills/upstream-version-watch/scripts/render_dockerfile.sh").to_string_lossy().into_owned();
    let p = run(&["bash", &l, "--cluster-envfile", "--manifest", &m.to_string_lossy()], None, &[], None, Some(120))?;
    if p.code != 0 {
        return Err(Fail::new("ValueError", format!("render_cluster_envfile failed (rc={}): {}", p.code, strip(&p.stderr))));
    }
    Ok(p.stdout)
}

pub fn predicate_MODEL_TRIPLET_NO_SUB_PROPAGATION_C2() -> R {
    let compose = rendered("compose")?;
    let slave_block = search("(?s)vllm-slave-serve:.*?(?=\\n  vllm|\\z)", &compose).map(|m| m.2)
        .ok_or_else(|| Fail::new("AttributeError", "'NoneType' object has no attribute 'group'"))?;
    let env_files = findall("- envs/(\\.env\\.\\S+)", &slave_block);
    req(env_files == [".env.interconnect", ".env.cluster"], format!("slave env_file list must be exactly [.env.interconnect, .env.cluster], got {}", repr_list(&env_files)))?;
    let rendered_env = cluster_envfile("nodes:\n  - role: main\n    host: 10.0.0.1\n    ssh_user: u\n  - role: sub\n    host: 10.0.0.2\ninterconnect:\n  platform_preset: dgx-spark-gb10\n")?;
    let max_jobs = splitlines(&rendered_env).into_iter().filter(|l| !l.starts_with('#')).find_map(|l| l.strip_prefix("MAX_JOBS=").map(String::from));
    req(max_jobs.as_deref() == Some("4"), "MoE-JIT MAX_JOBS must be part of the Band2 cluster env the slave loads")?;
    req(rendered_env.contains("MAX_JOBS=4"), "predicate requirement failed at original line 660")
}

/// `slave_forward` through its command line (`slave_forward.sh`).
fn slave_forward(args: &[&str]) -> R<Proc> {
    let l = repo_root().join(".claude/skills/upstream-version-watch/scripts/slave_forward.sh").to_string_lossy().into_owned();
    let mut argv = vec!["bash", l.as_str()];
    argv.extend_from_slice(args);
    run(&argv, None, &[], None, Some(60))
}

/// `sf.prefix(sf.derive(ef, project, compose), *groups)`.
fn sf_prefix(ef: &Path, project: &Path, compose: &Path, groups: &str) -> R<String> {
    let p = slave_forward(&["prefix", "--cell-env", &ef.to_string_lossy(), "--project-env", &project.to_string_lossy(), "--compose", &compose.to_string_lossy(), "--group", groups])?;
    if p.code != 0 {
        return Err(Fail::new("ForwardError", strip(&p.stderr).to_string()));
    }
    Ok(strip(&p.stdout).to_string())
}

pub fn predicate_VARIANT_IMAGE_BUILD_VS_SERVE_PLANE_C1() -> R {
    let mn = read(".claude/skills/upstream-version-watch/scripts/multinode_serve_smoke.sh")?;
    req(mn.contains("val(){ grep -E \"^$1=\" \"$EF\" | head -1 | cut -d= -f2-; }"), "image-identity vars must be extracted from the SAME model env file ($EF) master build/up loads")?;
    req(mn.contains("IMG=$(val IMAGE_TAG)"), "the master-side IMG must still be read from the same model env file ($EF)")?;
    req(mn.contains("SF=\"$SDIR/slave_forward.sh\""), "the smoke must bind the single-owner forward derivation (slave_forward launcher) next to itself")?;
    req(mn.contains("COMPOSE_MULTI=\"output/multi/docker-compose.yaml\"") && mn.contains("--compose \"$COMPOSE_MULTI\"") && mn.contains("--cell-env \"$EF\""),
        "the forward set must be derived from the SAME compose and the SAME model env file the smoke runs with")?;
    let assign = splitlines(&mn).into_iter().find(|l| l.starts_with("SLAVE_IMGVARS=")).unwrap_or("");
    req(assign.contains("\"$SF\" prefix --group image_identity,cluster"), "SLAVE_IMGVARS must be assigned from slave_forward.sh (image_identity,cluster) -- no hand-written list")?;
    req(!mn.contains("${IMG:+IMAGE_TAG="), "the retired hand-written SLAVE_IMGVARS list must not come back")?;
    let find = |needle: &str| splitlines(&mn).into_iter().find(|l| l.contains(needle)).ok_or_else(|| Fail::new("StopIteration", ""));
    req(find("--profile slave build")?.contains("$SLAVE_IMGVARS"), "slave build invocation must carry the image-identity vars")?;
    let (master_build, master_up, slave_up) = (find("--profile master build")?, find("--profile master up")?, find("--profile slave up")?);
    req(master_build.contains("--env-file \"$EF\""), "master must build from the same model env file the vars came from")?;
    req(master_up.contains("--env-file \"$EF\""), "master must also RUN (not just build) from that same env file")?;
    req(slave_up.contains("$SLAVE_IMGVARS"), "slave must also RUN (not just build) with the same image-identity vars")?;

    let compose_text = rendered("compose")?;
    let base_env = "IMAGE_TAG=easy-vllm:0.25.1-cu132-aarch64-source-sm12x\nVLLM_REPO=https://github.com/jasl/vllm.git\nVLLM_REF=b5c0d43b967c\nBUILD_DOCKERFILE=Dockerfile.source-build\nVLLM_PRETEND_VERSION=0.26.1\n";
    let variant_ids: BTreeSet<String> = ["IMAGE_TAG=easy-vllm:0.25.1-cu132-aarch64-source-sm12x", "BUILD_DOCKERFILE=Dockerfile.source-build",
        "VLLM_REPO=https://github.com/jasl/vllm.git", "VLLM_PRETEND_VERSION=0.26.1", "VLLM_REF=b5c0d43b967c"].iter().map(|s| s.to_string()).collect();
    let tmp = TempDir::new("tmp")?;
    let compose = tmp.path().join("docker-compose.yaml");
    write(&compose, &compose_text)?;
    let project = tmp.path().join("project.env");
    write(&project, "")?;
    let ef = tmp.path().join("combo.env");
    let imgvars = |env_text: &str, compose_path: &Path| -> R<BTreeSet<String>> {
        write(&ef, env_text)?;
        Ok(sf_prefix(&ef, &project, compose_path, "image_identity")?.split_whitespace().map(String::from).collect())
    };
    // structural `sf.candidates(compose)`: a compose reference's group does not depend on its value, so the group the
    // derivation assigns each probed key (all set to the same safe value) is its candidate group.
    let probe_keys = ["IMAGE_TAG", "BUILD_DOCKERFILE", "VLLM_VERSION", "VLLM_REPO", "VLLM_REF", "VLLM_PRETEND_VERSION", "SM12X_PORT", "SRC_DEPS_AUTHORITY", "BUILD_JOBS", "CONFIG_FILE"];
    write(&ef, &probe_keys.iter().map(|k| format!("{k}=1\n")).collect::<String>())?;
    let j = slave_forward(&["json", "--cell-env", &ef.to_string_lossy(), "--project-env", &project.to_string_lossy(), "--compose", &compose.to_string_lossy()])?;
    req(j.code == 0, format!("slave_forward json failed: {}", strip(&j.stderr)))?;
    let doc = json_loads(&j.stdout)?;
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in jlist(jget(&doc, "forwards")?) {
        if let (Some(k), Some(g)) = (jget(&f, "key").ok().and_then(jstr), jget(&f, "group").ok().and_then(jstr)) {
            groups.entry(g).or_default().push(k);
        }
    }
    let in_group = |g: &str, k: &str| groups.get(g).map_or(false, |v| v.iter().any(|x| x == k));
    for key in ["IMAGE_TAG", "BUILD_DOCKERFILE", "VLLM_VERSION", "VLLM_REPO", "VLLM_REF", "VLLM_PRETEND_VERSION", "SM12X_PORT", "SRC_DEPS_AUTHORITY"] {
        req(in_group("image_identity", key), format!("{key} must be derived as cluster-wide image identity from the compose"))?;
    }
    req(in_group("build_tuning", "BUILD_JOBS") && !in_group("image_identity", "BUILD_JOBS"), "BUILD_JOBS is build tuning (same output, different parallelism), never image identity")?;
    req(groups.values().all(|keys| !keys.iter().any(|k| k == "CONFIG_FILE")), "CONFIG_FILE must never be forwarded -- the slave stays Band2-only")?;
    let mut with_port = variant_ids.clone();
    with_port.insert("SM12X_PORT=1".into());
    req(imgvars(&format!("{base_env}SM12X_PORT=1\n"), &compose)? == with_port, "the slave must receive the exact SM12X_PORT gate value the master reads from $EF")?;
    let stock_out = imgvars(base_env, &compose)?;
    req(stock_out == variant_ids, format!("stock combo must forward exactly the identity it declares: {}", repr_list(&stock_out.iter().collect::<Vec<_>>())))?;
    req(!imgvars(&format!("{base_env}SM12X_PORT=\n"), &compose)?.iter().any(|t| t.starts_with("SM12X_PORT")), "an empty SM12X_PORT must vanish from SLAVE_IMGVARS entirely, never leak as an empty assignment")?;
    let wheel_out = imgvars("IMAGE_TAG=easy-vllm:0.19.0-cu130-aarch64-wheel\nBUILD_DOCKERFILE=Dockerfile\nVLLM_VERSION=0.19.0\n", &compose)?;
    req(wheel_out.contains("VLLM_VERSION=0.19.0"), "the slave must receive the exact VLLM_VERSION the master reads from $EF -- otherwise the same IMAGE_TAG carries a different engine on each node")?;
    req(!stock_out.iter().any(|t| t.starts_with("VLLM_VERSION")), "an absent VLLM_VERSION must vanish from SLAVE_IMGVARS entirely, never leak as an empty assignment")?;
    req(imgvars(&format!("{base_env}SRC_DEPS_AUTHORITY=1\n"), &compose)?.contains("SRC_DEPS_AUTHORITY=1"), "the slave must receive the SRC_DEPS_AUTHORITY dependency-authority gate the master reads from $EF")?;
    write(&ef, &format!("{base_env}CONFIG_FILE=some-model\n"))?;
    req(!sf_prefix(&ef, &project, &compose, "image_identity,build_tuning,cluster,mount,serve_env")?.contains("CONFIG_FILE"), "the per-model CONFIG_FILE must never reach the slave through the env prefix")?;
    let grown = tmp.path().join("grown-compose.yaml");
    req(compose_text.contains("      SM12X_PORT: ${SM12X_PORT:-0}\n"), "rendered compose must declare the SM12X_PORT build-arg")?;
    write(&grown, &compose_text.replacen("      SM12X_PORT: ${SM12X_PORT:-0}\n", "      SM12X_PORT: ${SM12X_PORT:-0}\n      PREDICATE_NEW_ARG: ${PREDICATE_NEW_ARG:-0}\n", 1))?;
    req(imgvars(&format!("{base_env}PREDICATE_NEW_ARG=1\n"), &grown)?.contains("PREDICATE_NEW_ARG=1"), "a build-arg newly added to the compose must be forwarded to the slave without any code change")
}
