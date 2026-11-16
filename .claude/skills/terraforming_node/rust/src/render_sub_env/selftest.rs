//! `render_sub_env.py --self-test`: the fixture render regression (no hardware, no real manifest). Same checks and lines
//! as the original, with the delivery-list assertions following the Rust launchers (see the module documentation).

use super::*;
use sha2::{Digest, Sha256};

pub const FIXTURE_MANIFEST: &str = "topology: multi
cpu_arch: \"aarch64\"
cuda_version: \"132\"
gpus_per_node: 1
gpu_model: \"TEST-GPU\"
nas_model_path: /srv/test-models
model_source: nas
interconnect:
  type: RoCE v2
  hca_devices: [testhca0, testhca1]
  gid_index: 3
  socket_iface: testif0
  mtu: 9000
  bandwidth_gbps: 200
  platform_preset: test-preset
nodes:
  - role: main
    host: 203.0.113.10
    ssh_user: tester
    work_dir: /srv/tester/ws
  - role: sub
    host: 203.0.113.11
    ssh_user: tester
    work_dir: /srv/tester/ws
";

fn pf(b: bool) -> &'static str {
    if b {
        "PASS"
    } else {
        "FAIL"
    }
}
fn tf(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

fn mkdtemp(prefix: &str) -> String {
    let base = std::env::temp_dir();
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    for k in 0..10000u32 {
        let p = base.join(format!("{prefix}{:08x}", seed.wrapping_add(k.wrapping_mul(2654435761)) ^ std::process::id()));
        if std::fs::create_dir(&p).is_ok() {
            return p.display().to_string();
        }
    }
    panic!("no temporary directory");
}

fn strs_at(v: &PyValue, path: &[&str]) -> Vec<String> {
    let mut cur = v.clone();
    for k in path {
        cur = crate::py::get(&cur, k).cloned().unwrap_or(PyValue::None);
    }
    pv_strs(Some(&cur))
}

fn glob_names(dir: &str, want_dir_with: Option<&str>) -> Vec<String> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with('.') {
                continue;
            }
            let p = pj(dir, &n);
            match want_dir_with {
                Some(f) => {
                    if isdir(&p) && isfile(&pj(&p, f)) {
                        out.push(n);
                    }
                }
                None => out.push(n),
            }
        }
    }
    out
}

fn rglob_files(dir: &str) -> Vec<String> {
    let mut out = vec![];
    if !isdir(dir) {
        return out;
    }
    for (root, _, files) in walk(dir) {
        for f in files {
            let p = pj(&root, &f);
            if isfile(&p) {
                out.push(p);
            }
        }
    }
    out
}

fn all_files(root: &str) -> Vec<String> {
    let mut v = vec![];
    for (dp, _, fs) in walk(root) {
        for f in fs {
            v.push(pj(&dp, &f));
        }
    }
    v
}

fn bytes_eq(a: &str, b: &str) -> Res<bool> {
    Ok(read_bytes(a)? == read_bytes(b)?)
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

fn set_eq(a: &[String], b: &[String]) -> bool {
    let mut x: Vec<&String> = a.iter().collect();
    let mut y: Vec<&String> = b.iter().collect();
    x.sort();
    x.dedup();
    y.sort();
    y.dedup();
    x == y
}

pub fn self_test(ctx: &Ctx) -> i32 {
    match body(ctx) {
        Ok(ok) => {
            println!("self-test: {}", pf(ok));
            if ok {
                0
            } else {
                1
            }
        }
        Err(f) => f.finish(),
    }
}

fn body(ctx: &Ctx) -> Res<bool> {
    let mut ok = true;
    let tmp = mkdtemp("render_sub_env_selftest_");
    let mpath = pj(&tmp, "manifest.yaml");
    write_text(&mpath, FIXTURE_MANIFEST)?;
    let phs = |m: &Man| build_placeholders(ctx, m, None);

    // (1)
    let data = parse_manifest(&mpath)?;
    let (ph, missing) = phs(&data)?;
    let g = |p: &Ph, k: &str| p.get(k).cloned().unwrap_or_default();
    let c1 = missing.is_empty()
        && g(&ph, "SUB_HOST") == "203.0.113.11"
        && g(&ph, "MASTER_HOST") == "203.0.113.10"
        && g(&ph, "INTERCONNECT_IFACE") == "testif0"
        && g(&ph, "INTERCONNECT_MTU") == "9000"
        && g(&ph, "GPU_MODEL") == "TEST-GPU"
        && g(&ph, "NAS_MOUNT") == "/srv/test-models"
        && g(&ph, "TOPOLOGY") == "multi";
    println!(
        "  [{}] manifest 파싱 + placeholders (missing={}, topology={})",
        pf(c1),
        list_repr(&missing),
        g(&ph, "TOPOLOGY")
    );
    ok &= c1;

    // (2)
    let mut data2 = data.clone();
    data2.pop("gpu_model");
    let mut ic = data.section("interconnect");
    ic.retain(|(k, _)| k != "mtu");
    data2.set("interconnect", MVal::Map(ic));
    let (ph2, missing2) = phs(&data2)?;
    let c2 = g(&ph2, "INTERCONNECT_MTU") == "9000" && g(&ph2, "GPU_MODEL").is_empty() && missing2.iter().any(|m| m == "GPU_MODEL");
    println!(
        "  [{}] mtu 폴백 9000 유지 · gpu_model 누락은 fail-loud(missing 에 GPU_MODEL) → got mtu={} gpu={} missing={}",
        pf(c2),
        g(&ph2, "INTERCONNECT_MTU"),
        PyStr::new(&g(&ph2, "GPU_MODEL")).repr(),
        list_repr(&missing2)
    );
    ok &= c2;

    // (3)
    let data3 = Man(vec![
        ("interconnect".into(), MVal::Map(vec![])),
        (
            "nodes".into(),
            MVal::List(vec![vec![
                ("role".into(), MVal::Str("sub".into())),
                ("host".into(), MVal::Str(String::new())),
                ("work_dir".into(), MVal::Str(String::new())),
            ]]),
        ),
    ]);
    let (_, missing3) = phs(&data3)?;
    let c3 = missing3.iter().any(|m| m == "SUB_HOST") && missing3.iter().any(|m| m == "MASTER_HOST");
    println!(
        "  [{}] 필수 누락 감지(fail-loud) → missing={}...",
        pf(c3),
        list_repr(&missing3[..missing3.len().min(3)])
    );
    ok &= c3;

    // (4)
    let out = pj(&tmp, "sub_provision");
    let r4: Res<()> = (|| {
        render_tree(ctx, &ph, &out, false, None, None, None)?;
        let mut base_expect: Vec<String> = [
            "CLAUDE.md", "AGENTS.md", "Agent_Card.json", ".claude/settings.local.json", ".codex/config.toml",
            ".codex/rules/sub.rules", ".claude/rules/comms.md", ".claude/schemas/task-report.schema.json",
            "campaigns/README.md", ".gitignore", ".claude/rules/strategy.topology.md",
            ".claude/runtime/host_safety/mem_watchdog.sh", ".claude/runtime/host_safety/install_host_safety.sh",
            ".claude/runtime/host_safety/install_netconsole.sh",
            ".claude/runtime/host_safety/systemd/easy-vllm-memwatch.service",
            ".claude/runtime/host_safety/host/vllm-drop-caches.sh",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        base_expect.extend(BLACKBOX_FILES.iter().map(|(f, _)| format!(".claude/runtime/node_blackbox/{f}")));
        let mut missing_art: Vec<String> =
            base_expect.iter().filter(|p| !exists(&pj(&out, p))).cloned().collect();
        let have = missing_art.is_empty();
        let rendered: Vec<String> = glob_names(&pj(&out, "docs"), Some("example.md"));
        let docs_ok = DOC_TYPES.iter().all(|d| rendered.iter().any(|r| r == d));
        if !docs_ok {
            let mut miss: Vec<&str> = DOC_TYPES.iter().copied().filter(|d| !rendered.iter().any(|r| r == d)).collect();
            miss.sort();
            missing_art.push(format!("docs/{{{}}}/example.md", miss.join(",")));
        }
        let mut leftover: Vec<String> = vec![];
        for p in ["CLAUDE.md", "AGENTS.md", "Agent_Card.json", ".claude/settings.local.json", ".codex/config.toml"] {
            leftover.extend(unrendered(&read_text(&pj(&out, p))?));
        }
        for p in ["Agent_Card.json", ".claude/settings.local.json", ".claude/schemas/task-report.schema.json"] {
            load_json(&pj(&out, p))?;
        }
        let st = load_json(&pj(&out, ".claude/settings.local.json"))?;
        let allow = strs_at(&st, &["permissions", "allow"]);
        let deny = strs_at(&st, &["permissions", "deny"]);
        let has = |l: &[String], s: &str| l.iter().any(|x| x == s);
        let git_ok = has(&allow, "Bash(git commit:*)")
            && has(&allow, "Bash(git checkout:*)")
            && !has(&deny, "Bash(git commit:*)")
            && ["push", "pull", "fetch", "remote"].iter().all(|r| has(&deny, &format!("Bash(git {r}:*)")))
            && has(&allow, "Bash(git clone:*)")
            && !has(&deny, "Bash(git clone:*)")
            && [
                "Bash(git clone https://huggingface.co:*)",
                "Bash(git clone https://hf.co:*)",
                "Bash(git clone *huggingface.co*)",
                "Bash(git clone *hf.co*)",
                "Bash(git lfs:*)",
            ]
            .iter()
            .all(|d| has(&deny, d));
        let c4 = have && leftover.is_empty() && git_ok && docs_ok;
        println!(
            "  [{}] 전체 렌더(미치환={}, 누락아티팩트={}, git권한정합={}, docs계약5종={})",
            pf(c4),
            list_repr(&leftover),
            list_repr(&missing_art),
            tf(git_ok),
            list_repr(&sorted(rendered))
        );
        ok &= c4;
        Ok(())
    })();
    match r4 {
        Ok(()) => {}
        Err(f @ Fail::Sys(_)) => {
            println!("  [FAIL] 렌더 예외: {}", f.text());
            ok = false;
        }
        Err(f) => return Err(f),
    }

    // (5)
    let mut data5 = parse_manifest(&mpath)?;
    data5.set("topology", MVal::Str("single".into()));
    let (ph5, _) = phs(&data5)?;
    let out5 = pj(&tmp, "sub_provision_single");
    let a2a_marks = ["A2A 원격 에이전트", "AgentCard"];
    let r5: Res<()> = (|| {
        render_tree(ctx, &ph5, &out5, false, None, None, None)?;
        let claude_single = read_text(&pj(&out5, "CLAUDE.md"))?;
        let ray_marks = ["slave(Ray worker)", "Ray head", "RAY_PORT", "/dev/infiniband", "--profile slave"];
        let leaked: Vec<&str> = ray_marks.iter().copied().filter(|m| claude_single.contains(m)).collect();
        let has_a2a = a2a_marks.iter().all(|m| claude_single.contains(m));
        let no_markers = !claude_single.contains("<!-- MODE:");
        let c5 = g(&ph5, "TOPOLOGY") == "single" && unrendered(&claude_single).is_empty() && leaked.is_empty() && has_a2a && no_markers;
        println!(
            "  [{}] single 렌더 페르소나=a2a-agent (ray누수={} a2a문장={} 마커제거={})",
            pf(c5),
            if leaked.is_empty() { "없음".to_string() } else { list_repr(&leaked) },
            tf(has_a2a),
            tf(no_markers)
        );
        ok &= c5;
        let mut data5m = parse_manifest(&mpath)?;
        data5m.set("topology", MVal::Str("multi".into()));
        let (ph5m, _) = phs(&data5m)?;
        let out5m = pj(&tmp, "sub_provision_multi_persona");
        render_tree(ctx, &ph5m, &out5m, false, None, None, None)?;
        let claude_multi = read_text(&pj(&out5m, "CLAUDE.md"))?;
        let c5b = claude_multi.contains("slave(Ray worker)")
            && !a2a_marks.iter().any(|m| claude_multi.contains(m))
            && !claude_multi.contains("<!-- MODE:");
        println!("  [{}] multi 렌더 페르소나=ray-worker (대조군 — 양방향 확인)", pf(c5b));
        ok &= c5b;
        Ok(())
    })();
    match r5 {
        Ok(()) => {}
        Err(f @ Fail::Sys(_)) => {
            println!("  [FAIL] single 렌더 예외: {}", f.text());
            ok = false;
        }
        Err(f) => return Err(f),
    }

    // (4c) permission plane per identity
    let kit = "output/**/Dockerfile";
    for (topo, want_allow, label) in
        [("single", true, "a2a-agent=자율저작 allow"), ("multi", false, "ray-worker=배달분 보호 deny")]
    {
        let mut d = parse_manifest(&mpath)?;
        d.set("topology", MVal::Str(topo.into()));
        let (p, _) = phs(&d)?;
        let o = pj(&tmp, &format!("perm_{topo}"));
        render_tree(ctx, &p, &o, false, None, None, None)?;
        let st = load_json(&pj(&o, ".claude/settings.local.json"))?;
        let al = strs_at(&st, &["permissions", "allow"]);
        let dn = strs_at(&st, &["permissions", "deny"]);
        let ends = format!("/{kit})");
        let in_allow = al.iter().any(|x| x.ends_with(&ends) && x.starts_with("Edit("));
        let in_deny = dn.iter().any(|x| x.ends_with(&ends) && x.starts_with("Edit("));
        let both: Vec<&String> = al.iter().chain(dn.iter()).collect();
        let no_write = !both.iter().any(|x| x.starts_with("Write("));
        let no_marker = !both.iter().any(|x| x.starts_with("MODE:"));
        let ws = g(&p, "WORKSPACE_PATH");
        let mut common = dn.iter().any(|x| x == "Bash(git push:*)") && dn.iter().any(|x| *x == format!("Edit({ws}/.claude/**)"));
        common &= ["AGENTS.md", ".agents/**", ".codex/**"].iter().all(|q| dn.iter().any(|x| *x == format!("Edit({ws}/{q})")));
        let codex_text = String::from_utf8_lossy(&read_bytes(&pj(&o, ".codex/config.toml"))?).into_owned();
        let codex = toml::loads(&codex_text).map_err(|e| Fail::Raise(Raised::new("tomllib.TOMLDecodeError", &e)))?;
        let codex_ok = codex.get("default_permissions") == Some(&toml::Toml::Str(":danger-full-access".into()))
            && codex.get("approval_policy") == Some(&toml::Toml::Str("never".into()))
            && !["permissions", "sandbox_mode", "sandbox_workspace_write"].iter().any(|k| codex.get(k).is_some());
        let c = (in_allow == want_allow) && (in_deny != want_allow) && no_marker && common && no_write && codex_ok;
        println!(
            "  [{}] 권한 평면 {topo}({label}): allow={} deny={} 마커제거={} 공통보존={} Write(path)부재={} Codex역할권한={}",
            pf(c),
            tf(in_allow),
            tf(in_deny),
            tf(no_marker),
            tf(common),
            tf(no_write),
            tf(codex_ok)
        );
        ok &= c;
    }
    let mut neg = PyDict::new();
    neg.set("a", PyValue::List(vec![PyValue::str("MODE:nonesuch X")]));
    match apply_mode_prefixes(&PyValue::Dict(neg), "a2a-agent") {
        Ok(_) => {
            println!("  [FAIL] 미지 MODE 접두가 통과했다");
            ok = false;
        }
        Err(f @ Fail::Sys(_)) => {
            println!("  [PASS] 미지 MODE 접두 → fail-loud ({}…)", crate::py::head(&f.text(), 40));
        }
        Err(f) => return Err(f),
    }

    // (4b) tool_plane gating
    for (topo, want, label) in [
        (
            "single",
            vec!["vllm-recipe-explorer", "adversarial-benchmark", "upstream-version-watch", "hint-publisher", "wiki-desk"],
            "a2a-agent=3종+hint-publisher+wiki-desk",
        ),
        ("multi", vec!["wiki-desk"], "ray-worker=wiki-desk"),
    ] {
        let want: Vec<String> = want.into_iter().map(String::from).collect();
        let mut d = parse_manifest(&mpath)?;
        d.set("topology", MVal::Str(topo.into()));
        let (p, _) = phs(&d)?;
        let o = pj(&tmp, &format!("tp_{topo}"));
        render_tree(ctx, &p, &o, true, None, None, None)?;
        let skills_dir = pj(&o, ".claude/skills");
        let got: Vec<String> = glob_names(&skills_dir, Some("SKILL.md"));
        let mut ronly_ok = true;
        for (rn, manifest) in RUST_OWNER_MANIFESTS {
            let rd = pj(&skills_dir, rn);
            let has_rust = isfile(&pj(&o, manifest));
            let full = isfile(&pj(&rd, "SKILL.md"));
            let extra = ["references", "assets"].iter().any(|x| exists(&pj(&rd, x)));
            let wr = exists(&pj(&o, &format!(".agents/skills/{rn}")));
            if !has_rust || full != want.iter().any(|w| w == rn) || (!full && (extra || wr)) {
                ronly_ok = false;
            }
            for inp in rust_only_inputs(rn) {
                let ip = pj(&rd, inp.trim_end_matches('/'));
                let src_ip = pj(&ctx.repo, &format!(".claude/skills/{rn}/{}", inp.trim_end_matches('/')));
                if inp.ends_with('/') {
                    let fs = rglob_files(&ip);
                    if fs.is_empty() {
                        ronly_ok = false;
                    }
                    for f in fs {
                        let rel = relpath(&f, &ip);
                        if read_bytes(&f)? != read_bytes(&pj(&src_ip, &rel))? {
                            ronly_ok = false;
                        }
                    }
                } else if !(isfile(&ip) && bytes_eq(&ip, &src_ip)?) {
                    ronly_ok = false;
                }
            }
        }
        let parity_ok = isfile(&pj(&o, &format!("{PARITY_RUST_TOOL}/Cargo.toml")))
            && isdir(&pj(&o, &format!("{PARITY_RUST_TOOL}/data")))
            && isfile(&pj(&o, &format!("{PARITY_RUST_TOOL}/cases/s1.json")));
        let refs = exists(&pj(&o, ".claude/skills/wiki-desk/reference/references.md"));
        let ref_ok = refs == want.iter().any(|w| w == "wiki-desk");
        let shared_rust_ok = shared_rust_libraries().iter().all(|s| isfile(&pj(&o, &format!("{s}/Cargo.toml"))));
        let wrappers = glob_names(&pj(&o, ".agents/skills"), Some("SKILL.md"));
        let mut wrapper_ok = set_eq(&wrappers, &want);
        for name in &wrappers {
            let wp = pj(&o, &format!(".agents/skills/{name}/SKILL.md"));
            let link = format!("../../../.claude/skills/{name}/SKILL.md");
            wrapper_ok &= read_text(&wp)?.contains(&link) && isfile(&pj(&dirname(&wp), &link));
        }
        let tn = pj(&skills_dir, "terraforming_node/scripts");
        let tn_files = if isdir(&tn) {
            sorted(glob_all(&tn).into_iter().filter(|f| isfile(&pj(&tn, f))).collect())
        } else {
            vec![]
        };
        let mut tn_want: Vec<String> = delivered_tn_tools().iter().map(|s| s.to_string()).collect();
        if want.iter().any(|w| w == "hint-publisher") {
            tn_want.extend(SUB_HINT_EQUIP_TOOLS.iter().map(|s| s.to_string()));
        }
        tn_want.push("scan_node.sh".into());
        let tn_ok = tn_files == sorted(tn_want);
        let rust_helper = pj(&tn, "rust_bootstrap.sh");
        let rust_help = crate::proc::run(&["bash".into(), rust_helper.clone(), "--help".into()], None, Some(20))?;
        let rust_ok = access_x(&rust_helper)
            && rust_help.returncode == 0
            && isfile(&pj(&o, RUST_RUNTIME_REFERENCE))
            && !exists(&pj(&o, ".claude/runtime/rust"));
        // the delivered role-contract reader runs from the render tree (its launcher; the ELF of this repository's
        // upstream runtime, since a render tree is source only)
        let nrc = pj(&tn, "node_role_contract.sh");
        let nrc_ok = isfile(&nrc) && run_with_env(
            &["bash", &nrc, "--self-test"],
            &[("EASY_VLLM_UPSTREAM_RUNTIME_ROOT", &pj(&ctx.repo, ".claude/skills/upstream-version-watch/rust/runtime"))],
            60,
        )? == 0;
        let bench_need = [
            (".claude/policies/runtime/topology_parity.sh", ".claude/skills/adversarial-benchmark/rust/src/resolve_topology_selftest.rs"),
            (
                ".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh",
                ".claude/skills/adversarial-benchmark/rust/src/sweep_repeats_fixture.rs",
            ),
            (".claude/skills/terraforming_node/scripts/scan_node.sh", ".claude/skills/adversarial-benchmark/scripts/build_rust.sh"),
        ];
        let need_keys: Vec<String> = bench_need.iter().map(|(a, _)| a.to_string()).collect();
        let found: Vec<String> = SUB_BENCH_FOUNDATION_RUNTIME.iter().map(|s| s.to_string()).collect();
        let mut bench_ok = set_eq(&need_keys, &found);
        for (rel, consumer) in bench_need {
            let cons = read_text(&pj(&ctx.repo, consumer))?;
            let dst = pj(&o, rel);
            bench_ok &= cons.contains(rel) && isfile(&dst) && bytes_eq(&dst, &pj(&ctx.repo, rel))?;
        }
        let bscr = pj(&skills_dir, "adversarial-benchmark/scripts");
        let bskill = dirname(&bscr);
        let mut called_missing: Vec<String> = vec![];
        let mut sh: Vec<String> = glob_all(&bscr).into_iter().filter(|n| n.ends_with(".sh") && !n.starts_with('.')).collect();
        sh.sort();
        for sf in &sh {
            let txt = String::from_utf8_lossy(&read_bytes(&pj(&bscr, sf))?).into_owned();
            let mut refs: Vec<String> = vec![];
            refs.extend(textre::find_repo_refs(&txt).into_iter().map(|m| crate::pathfmt::py_path(&m)));
            let bskill_rel = relpath(&bskill, &o);
            refs.extend(textre::find_sdir_parent_refs(&txt).into_iter().map(|m| crate::pathfmt::py_path(&format!("{bskill_rel}/{m}"))));
            let bscr_rel = relpath(&bscr, &o);
            refs.extend(textre::find_sdir_scripts(&txt).into_iter().map(|m| crate::pathfmt::py_path(&format!("{bscr_rel}/{m}"))));
            for r in refs {
                if !exists(&pj(&o, &r)) {
                    called_missing.push(format!("{sf}→{r}"));
                }
            }
        }
        let rsrc = pj(&bskill, "rust/src");
        let mut rs: Vec<String> = glob_all(&rsrc).into_iter().filter(|n| n.ends_with(".rs") && !n.starts_with('.')).collect();
        rs.sort();
        for f in &rs {
            let txt = String::from_utf8_lossy(&read_bytes(&pj(&rsrc, f))?).into_owned();
            for m in textre::find_skill_sources(&txt) {
                if !exists(&pj(&skills_dir, &m)) {
                    called_missing.push(format!("{f}→skills/{m}"));
                }
            }
        }
        let mut deps: Vec<&str> = SUB_B_CHECK_INPUTS.to_vec();
        deps.extend(sub_b_evidence_deps());
        for rel in deps {
            let pth = pj(&o, rel);
            bench_ok &= isfile(&pth) && bytes_eq(&pth, &pj(&ctx.repo, rel))?;
        }
        bench_ok &= called_missing.is_empty();
        if !called_missing.is_empty() {
            println!(
                "  [FAIL-detail] 배달된 B 스크립트가 부르는데 렌더 트리에 없는 파일: {}",
                list_repr(&called_missing[..called_missing.len().min(10)])
            );
        }
        // permission canary: the delivered launcher, the crate that builds it, and the bootstrap reader (its launcher)
        let (probe_name, reader_name) = ("harness_permission_probe.sh", "manifest_contract.sh");
        let probe_dst = pj(&tn, probe_name);
        let reader_dst = pj(&tn, reader_name);
        let probe_text = if isfile(&probe_dst) { read_text(&probe_dst)? } else { String::new() };
        let crate_dst = pj(&o, &format!("{TERRAFORMING_RUST}/Cargo.toml"));
        let canon = pj(&ctx.repo, ".claude/skills/terraforming_node/scripts");
        let probe_ok = probe_text.contains("harness_permission_probe")
            && isfile(&crate_dst)
            && bytes_eq(&crate_dst, &pj(&ctx.repo, &format!("{TERRAFORMING_RUST}/Cargo.toml")))?
            && isfile(&reader_dst)
            && bytes_eq(&reader_dst, &pj(&canon, reader_name))?
            && bytes_eq(&probe_dst, &pj(&canon, probe_name))?;
        let res_leak = exists(&pj(&o, ".claude/skills/upstream-version-watch/assets/current-production-resolution.json"));
        let sup = support_delivery(ctx, &sorted(want.clone()))?;
        let mut real: Vec<String> = vec![];
        for f in all_files(&o) {
            let r = relpath(&f, &o);
            let pick = r.starts_with(".claude/skills/terraforming_node/")
                || r.starts_with(".claude/policies/")
                || r.starts_with("hints/")
                || HINT_ENGINE_DEPENDENCIES.contains(&r.as_str());
            if pick {
                real.push(if r.starts_with(&format!("{TERRAFORMING_RUST}/")) { format!("{TERRAFORMING_RUST}/") } else { r });
            }
        }
        let sup_ok = set_eq(&sup, &real);
        let never: Vec<&str> = HINT_NEVER_DELIVER.iter().copied().filter(|r| exists(&pj(&o, r))).collect();
        let got_wo: Vec<String> = got.iter().filter(|x| *x != "terraforming_node").cloned().collect();
        let c = set_eq(&got_wo, &want)
            && ref_ok
            && tn_ok
            && wrapper_ok
            && rust_ok
            && nrc_ok
            && bench_ok
            && probe_ok
            && ronly_ok
            && parity_ok
            && !res_leak
            && sup_ok
            && never.is_empty();
        println!(
            "  [{}] tool_plane 게이팅 {topo}({label}): 배달={} refs={} 캠페인도구={} 해소자산누수={} 지원배달목록일치={} 금지경로누수={} Codex발견폐포={} 역할계약리더자체검사={} 벤치owner의존={} 프로브리더의존={}",
            pf(c),
            list_repr(&sorted(got.clone())),
            tf(refs),
            list_repr(&tn_files),
            tf(res_leak),
            tf(sup_ok),
            if never.is_empty() { "없음".to_string() } else { list_repr(&never) },
            tf(wrapper_ok),
            tf(nrc_ok),
            tf(bench_ok),
            tf(probe_ok)
        );
        ok &= c && shared_rust_ok;
        println!("  [{}] shared Q/role conditional delivery/path closure {topo}", pf(shared_rust_ok));
        // no `.py` reaches a Sub (plan_26111615): every harness program is a launcher + crate sources built on the Sub
        let py: Vec<String> = sorted(all_files(&o).into_iter().filter(|f| is_retired_py(f)).map(|f| relpath(&f, &o)).collect());
        let no_py = py.is_empty();
        println!(
            "  [{}] 배달 트리 .py 0건 {topo}: {}",
            pf(no_py),
            if no_py { "없음".to_string() } else { list_repr(&py[..py.len().min(10)]) }
        );
        ok &= no_py;
    }

    // (4d) hint engine dependency closure
    // Derived from the Rust owners (plan_26111615): the `REL_*` constants of the hint engine (`hintlib` core + evidence) and
    // the schemas the completion gate / evidence publisher load. A `.py` constant names a retired original whose owner runs
    // its sibling `.sh` launcher (`core::owner_launcher`), so it is classified as that launcher.
    let hp = pj(&ctx.repo, ".claude/skills/hint-publisher/rust/src");
    let mut hint_sources = vec![pj(&hp, "core.rs")];
    let ev = pj(&hp, "evidence");
    let mut ev_files: Vec<String> = glob_all(&ev).into_iter().filter(|n| n.ends_with(".rs")).map(|n| pj(&ev, &n)).collect();
    ev_files.sort();
    hint_sources.extend(ev_files);
    let mut derived: Vec<String> = vec![];
    for f in &hint_sources {
        for rel in textre::find_rust_rel_constants(&read_text(f)?) {
            derived.push(match rel.strip_suffix(".py") {
                Some(stem) => format!("{stem}.sh"),
                None => rel,
            });
        }
    }
    for f in [".claude/rust/policy-gate/src/completion_gate.rs", ".claude/rust/policy-publisher/src/gate.rs"] {
        let t = read_text(&pj(&ctx.repo, f))?;
        derived.extend(textre::find_rust_load_schema(&t).into_iter().map(|m| format!(".claude/schemas/{m}")));
    }
    derived.sort();
    derived.dedup();
    let node_local = [".claude/pii_terms.txt", "docs/_evidence", "docs/logs", "sync_staging/sub_docs/logs"];
    let tp = role_contract::tool_plane(&PyValue::str("single"), &PyValue::str("a2a-agent"))
        .map(|v| pv_strs(Some(&v.value)))
        .unwrap_or_default();
    let single_rb: Vec<String> = tp.iter().map(|n| format!(".claude/skills/{n}")).collect();
    let ex: Vec<String> = UPSTREAM_EXCLUDES.iter().map(|x| format!(".claude/skills/upstream-version-watch/{x}")).collect();
    let mut tn_rel: Vec<String> = vec![];
    for t in CAMPAIGN_TOOLS.iter().chain(SUB_CONTRACT_READERS.iter()).chain(SUB_HINT_EQUIP_TOOLS.iter()) {
        tn_rel.push(tn_script(t));
    }
    let covered = |rel: &str| -> bool {
        if HINT_ENGINE_DEPENDENCIES.contains(&rel) || tn_rel.iter().any(|x| x == rel) || node_local.contains(&rel) {
            return true;
        }
        if HINT_NEVER_DELIVER.contains(&rel) || HINT_ENGINE_TOLERATED_ABSENT.contains(&rel) {
            return true;
        }
        // Sources of the delivered policy crates (the hint engine reads the gate's Rust API as text, e.g.
        // policy-gate/src/completion_gate.rs): the whole crate travels to every B Sub, so the path is delivered.
        if POLICY_RUST_LIBRARIES.iter().any(|c| rel.starts_with(&format!("{c}/"))) {
            return true;
        }
        single_rb.iter().any(|rb| rel == rb || rel.starts_with(&format!("{rb}/"))) && !ex.iter().any(|x| x == rel)
    };
    let unclassified: Vec<String> = derived.iter().filter(|r| !covered(r)).cloned().collect();
    let dep_exist: Vec<&str> = HINT_ENGINE_DEPENDENCIES.iter().copied().filter(|r| !isfile(&pj(&ctx.repo, r))).collect();
    let c4d = !derived.is_empty() && unclassified.is_empty() && dep_exist.is_empty();
    println!(
        "  [{}] hint 엔진 의존 폐포: 파생 {}경로 · 미분류={} · 의존목록 부재={}",
        pf(c4d),
        derived.len(),
        if unclassified.is_empty() { "없음".to_string() } else { list_repr(&unclassified) },
        if dep_exist.is_empty() { "없음".to_string() } else { list_repr(&dep_exist) }
    );
    ok &= c4d;
    // information, not a verdict of this renderer: constants that still name a retired original without a sibling launcher
    // are read by the hint engine as source text (its owner's port gap) — named here so the gap is not silent.
    let no_launcher: Vec<String> =
        derived.iter().filter(|r| r.ends_with(".sh") && !isfile(&pj(&ctx.repo, r))).cloned().collect();
    if !no_launcher.is_empty() {
        println!("  [INFO] hint 엔진 상수 중 런처 없는 은퇴 원본(엔진 소유 이식 공백 · 배달 대상 아님): {}", list_repr(&no_launcher));
    }

    // (4b-2) upstream path split
    let mut d = parse_manifest(&mpath)?;
    d.set("topology", MVal::Str("single".into()));
    let (p, _) = phs(&d)?;
    let o = pj(&tmp, "upstream_split");
    render_tree(ctx, &p, &o, true, None, None, None)?;
    let uw = pj(&o, ".claude/skills/upstream-version-watch");
    let leaked: Vec<&str> = UPSTREAM_EXCLUDES.iter().copied().filter(|x| exists(&pj(&uw, x))).collect();
    let needed = [
        "scripts/resolve_wheel.sh",
        "scripts/resolve_torch_pin.sh",
        "scripts/render_dockerfile.sh",
        "scripts/regen_requirements.sh",
        "scripts/classify_failure.sh",
        "references/resolve-and-render.md",
    ];
    let absent: Vec<&str> = needed.iter().copied().filter(|x| !exists(&pj(&uw, x))).collect();
    let canon = pj(&ctx.repo, ".claude/skills/upstream-version-watch/scripts");
    let mut unclassified: Vec<String> = glob_all(&canon)
        .into_iter()
        .filter(|n| {
            (n.ends_with(".sh") || n.ends_with(".py"))
                && ["sub", "branch", "multinode", "clone", "closing", "ledger", "push"].iter().any(|k| n.contains(k))
        })
        .map(|n| format!("scripts/{n}"))
        .filter(|c| !UPSTREAM_EXCLUDES.contains(&c.as_str()))
        .collect();
    unclassified.sort();
    unclassified.dedup();
    let c4b2 = leaked.is_empty() && absent.is_empty() && unclassified.is_empty();
    let none_or = |v: Vec<String>| if v.is_empty() { "없음".to_string() } else { list_repr(&v) };
    println!(
        "  [{}] upstream 경로 분할: 누수={} 필요분누락={} 미분류={}",
        pf(c4b2),
        none_or(leaked.iter().map(|s| s.to_string()).collect()),
        none_or(absent.iter().map(|s| s.to_string()).collect()),
        none_or(unclassified)
    );
    ok &= c4b2;

    // (4c) render determinism
    let d1 = parse_manifest(&mpath)?;
    let (ph1, _) = phs(&d1)?;
    let (o1, o2) = (pj(&tmp, "det_a"), pj(&tmp, "det_b"));
    render_tree(ctx, &ph1, &o1, true, None, None, None)?;
    render_tree(ctx, &ph1, &o2, true, None, None, None)?;
    let mut digest = vec![];
    for root in [&o1, &o2] {
        let mut h = Sha256::new();
        let mut files = all_files(root);
        files.sort();
        for f in files {
            if isfile(&f) {
                h.update(relpath(&f, root).as_bytes());
                h.update(read_bytes(&f)?);
            }
        }
        digest.push(h.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>());
    }
    let cdet = digest[0] == digest[1];
    println!("  [{}] 렌더 결정론(같은 입력 → 같은 바이트): {}…", pf(cdet), &digest[0][..16]);
    ok &= cdet;

    // (5c) runtime-block copy paths: git list · injected list · gitless tree fails loud
    let rb_rel = ".claude/skills/vllm-recipe-explorer";
    if isdir(&pj(&ctx.repo, rb_rel)) {
        let d5c = pj(&tmp, "rb_git");
        makedirs(&d5c)?;
        let (n_git, src_git) = copy_tracked(&ctx.repo, rb_rel, &d5c, None, None, None)?;
        let d5d = pj(&tmp, "rb_inject");
        makedirs(&d5d)?;
        let listed = git(&ctx.repo, &["ls-files", "-z"])?;
        let inject: Vec<String> = listed.stdout.split('\0').filter(|x| !pystrip(x).is_empty()).map(String::from).collect();
        let (n_inj, src_inj) = copy_tracked(&ctx.repo, rb_rel, &d5d, Some(&inject), None, None)?;
        let gitless_repo = pj(&tmp, "gitless_repo");
        let gitless = pj(&gitless_repo, rb_rel);
        makedirs(&gitless)?;
        write_text(&pj(&gitless, "SKILL.md"), "x\n")?;
        let failed_loud = match copy_tracked(&gitless_repo, rb_rel, &pj(&tmp, "rb_gitless"), None, None, None) {
            Ok(_) => false,
            Err(f @ Fail::Sys(_)) => {
                let _ = f;
                true
            }
            Err(f) => return Err(f),
        };
        let c5c = n_git > 0 && n_git == n_inj && src_git == "measured:git-ls-files" && src_inj == "injected:tracked-list" && failed_loud;
        println!(
            "  [{}] 런타임블럭 복제: git={n_git} 주입={n_inj} 출처={src_git}/{src_inj} git없는트리→fail-loud={}",
            pf(c5c),
            tf(failed_loud)
        );
        ok &= c5c;
    }

    // (6) no delegation key; identity assets present
    let mut data6 = parse_manifest(&mpath)?;
    if let Some(n) = data6.node_mut("sub") {
        map_set(n, "hw_verified", MVal::Str("true".into()));
    }
    let (ph6, _) = phs(&data6)?;
    let out6 = pj(&tmp, "sub_provision_identity");
    render_tree(ctx, &ph6, &out6, false, None, None, None)?;
    let key_gone = !exists(&pj(&out6, ".claude/a2a_delegation.json"));
    let card_ok = exists(&pj(&out6, "Agent_Card.json"));
    let c6 = key_gone && card_ok;
    println!("  [{}] 위임 키 폐기: 키 미생성({}) · 정체성 자산 존재({})", pf(c6), tf(key_gone), tf(card_ok));
    ok &= c6;

    // (7) topology axis 4 fields
    let data7 = parse_manifest(&mpath)?;
    let (ph7m, _) = phs(&data7)?;
    let out7m = pj(&tmp, "sub_provision_axis_multi");
    render_tree(ctx, &ph7m, &out7m, false, None, None, None)?;
    let role_params = |path: &str| -> Res<PyValue> {
        let card = load_json(path)?;
        let exts = match crate::py::get(&card, "capabilities").and_then(|c| crate::py::get(c, "extensions")).map(PyValue::peel) {
            Some(PyValue::List(l)) => l.clone(),
            _ => vec![],
        };
        let hits: Vec<PyValue> = exts
            .into_iter()
            .filter(|e| crate::py::get_str(e, "uri").as_deref() == Some(crate::agent_card::NODE_ROLE_EXT_URI))
            .collect();
        if hits.len() != 1 {
            let r = PyValue::List(hits.clone()).repr();
            return Err(sys(format!("[render self-test] node-role 확장이 정확히 1개여야 한다: {r}")));
        }
        Ok(crate::py::get(&hits[0], "params").cloned().unwrap_or(PyValue::None))
    };
    let card_m = role_params(&pj(&out7m, "Agent_Card.json"))?;
    let mut data7s = parse_manifest(&mpath)?;
    data7s.set("topology", MVal::Str("single".into()));
    let (ph7s, _) = phs(&data7s)?;
    let out7s = pj(&tmp, "sub_provision_axis_single");
    render_tree(ctx, &ph7s, &out7s, false, None, None, None)?;
    let card_s = role_params(&pj(&out7s, "Agent_Card.json"))?;
    let axis_keys = ["rank", "rank_source", "sub_mode", "sub_mode_source"];
    let has_key = |v: &PyValue, k: &str| matches!(v.peel(), PyValue::Dict(d) if d.contains_key(k));
    let keys_ok = axis_keys.iter().all(|k| has_key(&card_m, k)) && axis_keys.iter().all(|k| has_key(&card_s, k));
    let gv = |v: &PyValue, k: &str| crate::py::get(v, k).cloned().unwrap_or(PyValue::None);
    let is_s = |v: &PyValue, k: &str, s: &str| matches!(gv(v, k).peel(), PyValue::Str(x) if x.equals(s));
    let multi_ok = is_s(&card_m, "sub_mode", "ray-worker")
        && is_s(&card_m, "sub_mode_source", "derived-from-topology")
        && matches!(gv(&card_m, "rank").peel(), PyValue::Int(n) if n.to_string() == "1")
        && is_s(&card_m, "rank_source", "manifest-nodes-index");
    let single_ok = is_s(&card_s, "sub_mode", "a2a-agent")
        && matches!(gv(&card_s, "rank").peel(), PyValue::None)
        && is_s(&card_s, "rank_source", "not-applicable:single-a2a-agent");
    let owner_s = evaluate_contract_manifest(&data7s, Some("single"));
    let og = |sec: &str, k: &str| crate::py::get(&owner_s, sec).and_then(|x| crate::py::get(x, k)).cloned().unwrap_or(PyValue::None);
    let owner_ok = gv(&card_s, "sub_mode") == og("sub_mode", "value")
        && gv(&card_s, "sub_mode_source") == og("sub_mode", "source")
        && gv(&card_s, "rank") == og("rank", "value")
        && gv(&card_s, "rank_source") == og("rank", "source");
    let c7 = keys_ok && multi_ok && single_ok && owner_ok;
    println!(
        "  [{}] 토폴로지 축 4필드 탑재(keys={}, multi={}, single={}, 판정기일치={}) → multi rank={} / single rank={}",
        pf(c7),
        tf(keys_ok),
        tf(multi_ok),
        tf(single_ok),
        tf(owner_ok),
        gv(&card_m, "rank").repr(),
        gv(&card_s, "rank").repr()
    );
    ok &= c7;

    // (7b) Agent_Card metadata + sub manifest delivery
    let card_full = load_json(&pj(&out7s, "Agent_Card.json"))?;
    let mut v7b = crate::agent_card::validate_card(&card_full)?;
    v7b.extend(crate::agent_card::require_skills(&card_full)?);
    let std_ok = v7b.is_empty() && !has_key(&card_full, "node_identity") && !has_key(&card_full, "topology_contract");
    let sub_man_path = pj(&tmp, "sub_manifest.yaml");
    let sub_text = "self_role: sub\ntopology: single\ncpu_arch: aarch64\ngpus_per_node: 1\ngpu_model: SUB-GPU\nmodel_source: managed\nnas_model_path: /srv/test-models\nterraforming:\n  complete: true\n  branch_verified: true\n  issued_by: main\nnodes:\n  - role: main\n    host: 203.0.113.10\n  - role: sub\n    host: 203.0.113.11\n";
    write_text(&sub_man_path, sub_text)?;
    let sub_parsed = parse_manifest(&sub_man_path)?;
    let (ph7b, _) = build_placeholders(ctx, &data7s, Some(&sub_parsed))?;
    let out7b = pj(&tmp, "sub_provision_metadata");
    render_tree(ctx, &ph7b, &out7b, false, None, Some(&sub_man_path), None)?;
    let rendered_card = load_json(&pj(&out7b, "Agent_Card.json"))?;
    let unsigned_ok = !crate::py::get(&rendered_card, "signatures").map_or(false, |v| v.is_truthy());
    let trust_absent = !exists(&pj(&out7b, ".claude/a2a"));
    let sub_copied = isfile(&pj(&out7b, "output/single/manifest.yaml"));
    let hw_from_sub = g(&ph7b, "GPU_MODEL") == "SUB-GPU";
    let bad_sub = pj(&tmp, "sub_manifest_bad.yaml");
    write_text(&bad_sub, &read_text(&sub_man_path)?.replace("self_role: sub", "self_role: main"))?;
    let bad_caught = match render_tree(ctx, &ph7b, &pj(&tmp, "sub_provision_badsub"), false, None, Some(&bad_sub), None) {
        Ok(_) => false,
        Err(f @ Fail::Sys(_)) => f.text().contains("self_role"),
        Err(f) => return Err(f),
    };
    let c7b = std_ok && unsigned_ok && trust_absent && sub_copied && hw_from_sub && bad_caught;
    println!(
        "  [{}] Agent_Card metadata: 표준={} unsigned={} trust-store-부재={} 서브manifest={} HW출처={} self_role≠sub거부={}",
        pf(c7b),
        tf(std_ok),
        tf(unsigned_ok),
        tf(trust_absent),
        tf(sub_copied),
        tf(hw_from_sub),
        tf(bad_caught)
    );
    ok &= c7b;

    // (8) contract violation → fail-loud
    let mut data8 = parse_manifest(&mpath)?;
    data8.set("topology", MVal::Str("single".into()));
    if let Some(n) = data8.node_mut("sub") {
        map_set(n, "sub_mode", MVal::Str("ray-worker".into()));
    }
    let (ph8, missing8) = phs(&data8)?;
    let viol = g(&ph8, "SUB_CONTRACT_VIOLATIONS");
    let c8 = missing8.iter().any(|m| m == "SUB_MODE") && viol == "SUB_MODE_TOPOLOGY_CONFLICT";
    let mut m8 = missing8.clone();
    m8.sort();
    m8.dedup();
    println!(
        "  [{}] 계약 위반 → fail-loud(missing={}, violations={})",
        pf(c8),
        list_repr(&m8[..m8.len().min(3)]),
        PyStr::new(&viol).repr()
    );
    ok &= c8;

    // (9) topology rules self-declaration ↔ render channel
    let rules = pj(&tmp, "strategy.topology.md");
    let mut cases: Vec<bool> = vec![];
    for (hdr, topo, want_ok) in [("multi", "multi", true), ("single", "single", true), ("multi", "single", false), ("single", "multi", false)] {
        write_text(&rules, &format!("# s\n\n**topology: {hdr}** · layer: topology\n"))?;
        let why = topology_rules_mismatch(ctx, topo, Some(&rules));
        cases.push(why.is_none() == want_ok);
    }
    write_text(&rules, "# s\n\n선언 없음\n")?;
    let malformed = topology_rules_mismatch(ctx, "multi", Some(&rules)).is_some();
    let absent = topology_rules_mismatch(ctx, "multi", Some(&pj(&tmp, "absent.topology.md"))).is_some();
    let c9 = cases.iter().all(|x| *x) && malformed && absent;
    println!(
        "  [{}] 특화헌법 ↔ --topology 대조: 일치 2 통과 · 불일치 2 거부=[{}] · 선언 없음 거부={} · 부재 거부={}",
        pf(c9),
        cases.iter().map(|b| tf(*b)).collect::<Vec<_>>().join(", "),
        tf(malformed),
        tf(absent)
    );
    ok &= c9;

    let _ = std::fs::remove_dir_all(&tmp);
    Ok(ok)
}

fn glob_all(dir: &str) -> Vec<String> {
    match std::fs::read_dir(dir) {
        Ok(rd) => rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect(),
        Err(_) => vec![],
    }
}

fn access_x(p: &str) -> bool {
    use std::ffi::CString;
    let Ok(c) = CString::new(p) else { return false };
    extern "C" {
        fn access(path: *const std::os::raw::c_char, mode: i32) -> i32;
    }
    unsafe { access(c.as_ptr(), 1) == 0 }
}

/// `subprocess.run(argv, capture_output=True, text=True, timeout=t, env=dict(os.environ, **extra)).returncode`.
fn run_with_env(argv: &[&str], extra: &[(&str, &str)], timeout: u64) -> Res<i32> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(argv[0]);
    cmd.args(&argv[1..]).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in extra {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| Fail::Raise(Raised::os(&e, argv[0])))?;
    let start = std::time::Instant::now();
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let to = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = std::io::Read::read_to_end(&mut out, &mut b);
    });
    let te = std::thread::spawn(move || {
        let mut b = vec![];
        let _ = std::io::Read::read_to_end(&mut err, &mut b);
    });
    loop {
        if let Some(s) = child.try_wait().map_err(|e| Fail::Raise(Raised::new("OSError", &e.to_string())))? {
            let _ = to.join();
            let _ = te.join();
            return Ok(s.code().unwrap_or(-1));
        }
        if start.elapsed().as_secs() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Fail::Raise(Raised(format!(
                "subprocess.TimeoutExpired: Command {} timed out after {timeout} seconds",
                list_repr(argv)
            ))));
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hint_scope_paths() {
        assert!(super::inside("/a/b/c", "/a/b"));
        assert!(!super::inside("/a/bc", "/a/b"));
        assert!(super::iso_utc_zero("2026-11-16T00:00:00+00:00"));
        assert!(!super::iso_utc_zero("2026-11-16T00:00:00+09:00"));
        assert!(!super::iso_utc_zero("2026-11-16+00:00"));
        assert!(super::iso_utc_zero("2026-11-16T00:00:00.123+00:00"));
    }
}
