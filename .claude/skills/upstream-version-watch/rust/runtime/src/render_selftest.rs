//! `render_dockerfile.py --self-test` in Rust: the source-build key guard, the exit-① inheritance controls, the NCCL/cluster envfile goldens,
//! `env_tier`, the build-ledger stanza, the pre/post patch loop (run by POSIX `sh`) and the materializer, with the same fixtures and messages.
//!
//! Boundary (declared, not hidden): the ledger body is Python that runs INSIDE the image build. Its two interpreter-bound controls of the
//! original — `compile(body)` and running the assembler with the interpreter on raw captures — need a Python interpreter, which this
//! program never starts. They are reported as `N/A` lines below; the structural controls (ARG derivation, last-RUN placement, no
//! `import vllm`, new-ARG probe) and the shell-loop controls run here.

use super::*;
use crate::render_dockerfile::{
    build_cluster_env, build_nccl_env, env_tier, materialize_configs, render_cluster_envfile, render_nccl_envfile, IFACE_ENV_KEYS, RUNNER_SCRIPTS,
};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

type R = Result<(), String>;

fn req(cond: bool, msg: impl Into<String>) -> R {
    if cond {
        Ok(())
    } else {
        Err(msg.into())
    }
}

fn sv(t: &str) -> V {
    V::Str(t.to_string())
}

fn dv(items: Vec<(&str, V)>) -> V {
    V::Dict(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn lv(items: Vec<V>) -> V {
    V::List(items)
}

fn with_over(base: V, over: Vec<(&str, V)>) -> V {
    let V::Dict(mut d) = base else { return base };
    for (k, v) in over {
        match d.iter_mut().find(|(a, _)| a == k) {
            Some(slot) => slot.1 = v,
            None => d.push((k.to_string(), v)),
        }
    }
    V::Dict(d)
}

fn raised(r: Raised) -> String {
    format!("{}: {}", r.class, r.message)
}

const INHERIT_NGC: &str = "26.05-py3";
const INHERIT_VLLM: &str = "9.99.1";
const BASE_VLLM: &str = "0.26.0";

fn synthetic_attestation(over: Vec<(&str, V)>) -> V {
    let base = dv(vec![
        ("schema_version", V::Int("1".into())),
        ("from_ref", sv(&format!("v{BASE_VLLM}"))),
        ("to_ref", sv(&format!("v{INHERIT_VLLM}"))),
        ("provenance", sv("measured")),
        ("delta_source", sv("git-local")),
        ("axis_A_build_input", dv(vec![("verdict", sv("NO_IMPACT"))])),
        ("axis_B_port_scope", dv(vec![("verdict", sv("NO_IMPACT")), ("probes", lv(vec![])), ("silent_revert_risk", lv(vec![]))])),
        ("axis_C_model_path", dv(vec![("verdict", sv("NOT_IMPLEMENTED"))])),
        ("verdict", sv("NO_IMPACT")),
        ("unknown", lv(vec![])),
    ]);
    with_over(base, over)
}

fn entry(inherits: (&'static str, &'static str), approved_by: &'static str, attestation: &'static str) -> InheritEntry {
    InheritEntry { key: (INHERIT_NGC, INHERIT_VLLM), inherits, attestation, approved_by, approved_kst: "26082212" }
}

fn default_entry() -> InheritEntry {
    entry((INHERIT_NGC, BASE_VLLM), "self-test", "output/multi/resolved.json#upstream_delta")
}

fn res_of(vllm: &str, torch: &str, ngc: &str, cuda: &str, decision: &str, arch: Option<&str>, wheel: Option<V>) -> V {
    let mut items = vec![
        ("vllm_version", sv(vllm)),
        ("torch", dv(vec![("pin", sv(torch))])),
        ("ngc_base", dv(vec![("tag", sv(ngc)), ("cuda_version", sv(cuda))])),
        ("build_track", dv(vec![("decision", sv(decision))])),
        ("source_build", match arch {
            Some(a) => dv(vec![("torch_cuda_arch", sv(a))]),
            None => dv(vec![]),
        }),
    ];
    if let Some(w) = wheel {
        items.push(("wheel", w));
    }
    dv(items)
}

fn parse_env_pairs(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let (k, v) = l.split_once('=').unwrap_or((l, ""));
        out.insert(k.trim().to_string(), v.trim().to_string());
    }
    out
}

fn inherit_self_test(tpl: &str, man: &V) -> R {
    let render_with = |ledger: &[InheritEntry], att: Option<V>| -> Result<String, String> {
        let mut res = res_of(INHERIT_VLLM, "2.13.0", INHERIT_NGC, "13.2.0.046", "source-build", Some("12.1a"), None);
        if let Some(a) = att {
            res = with_over(res, vec![("upstream_delta", a)]);
        }
        let ctx = build_context_with(ledger, man, &res, false).map_err(raised)?;
        substitute(tpl, &ctx).map_err(raised)
    };
    let led = vec![default_entry()];
    let ok_out = render_with(&led, Some(synthetic_attestation(vec![])))?;
    req(ok_out.contains("INHERITED from") && !ok_out.contains("exit 1"), format!("양성 대조 실패 — 적격 상속인데 진행하지 않음:\n{ok_out}"))?;
    req(ok_out.contains("arbiter = smoke"), "상속 스탠자에 스모크 불변 문구 누락")?;
    req(ok_out.contains("axis_C=NOT_IMPLEMENTED"), "상속 스탠자가 axis_C 미구현 사실을 숨김")?;
    req(ok_out.contains("approved_by: self-test"), "상속 스탠자에 G1.6 승인자 미기재")?;
    req(ok_out.contains("silent_revert_risk=0"), format!("상속 스탠자의 basis 가 att 에서 파생되지 않음(하드코딩 회귀):\n{ok_out}"))?;
    let ok_b = render_with(
        &led,
        Some(synthetic_attestation(vec![
            ("axis_B_port_scope", dv(vec![("verdict", sv("IMPACT")), ("probes", lv(vec![dv(vec![("path", sv("vllm/x.py")), ("verdict", sv("NO_REVERT"))])])), ("silent_revert_risk", lv(vec![]))])),
            ("verdict", sv("IMPACT")),
        ])),
    )?;
    req(ok_b.contains("INHERITED from") && !ok_b.contains("exit 1"), format!("양성 대조② 실패 — axis_B=IMPACT ∧ silent_revert_risk=[] 인데 상속 거부:\n{ok_b}"))?;
    req(ok_b.contains("axis_B=IMPACT(silent_revert_risk=0)"), format!("양성 대조② 실패 — basis 가 실제 axis_B 상태를 숨김:\n{ok_b}"))?;
    let n4 = render_with(&led, None)?;
    req(n4.contains("exit 1") && n4.contains("NOT eligible"), format!("N4 실패 — 무증거 상속이 통과:\n{n4}"))?;
    req(n4.contains("attestation_present"), format!("N4 실패 — 거부 사유가 지목되지 않음:\n{n4}"))?;
    let n5 = render_with(&led, Some(synthetic_attestation(vec![("verdict", sv("UNDETERMINED")), ("unknown", lv(vec![sv("UNKNOWN_PLANE: newplane/x.toml")]))])))?;
    req(n5.contains("exit 1") && n5.contains("NOT eligible"), format!("N5 실패 — UNDETERMINED 상속이 통과:\n{n5}"))?;
    req(n5.contains("global_verdict") && n5.contains("unknown_empty"), format!("N5 실패 — UNDETERMINED/unknown 사유가 지목되지 않음:\n{n5}"))?;
    for axis in ["axis_A_build_input", "axis_B_port_scope"] {
        let o = render_with(&led, Some(synthetic_attestation(vec![(axis, dv(vec![("verdict", sv("UNDETERMINED"))]))])))?;
        req(o.contains("exit 1"), format!("N5' 실패 — {axis}=UNDETERMINED 인데 통과"))?;
    }
    let risky = dv(vec![
        ("verdict", sv("IMPACT")),
        ("probes", lv(vec![dv(vec![("path", sv("vllm/x.py")), ("verdict", sv("WOULD_REVERT"))])])),
        ("silent_revert_risk", lv(vec![dv(vec![("path", sv("vllm/x.py")), ("verdict", sv("WOULD_REVERT"))])])),
    ]);
    let neg: Vec<(&str, Vec<InheritEntry>, V)> = vec![
        ("axis_B.silent_revert_risk 비지 않음(재파생 필요)", led.clone(), synthetic_attestation(vec![("axis_B_port_scope", risky), ("verdict", sv("IMPACT"))])),
        ("axis_B 에 silent_revert_risk 필드 자체가 없음", led.clone(), synthetic_attestation(vec![("axis_B_port_scope", dv(vec![("verdict", sv("IMPACT"))])), ("verdict", sv("IMPACT"))])),
        ("attestation 이 다른 bump 의 것", led.clone(), synthetic_attestation(vec![("from_ref", sv("v0.1.0"))])),
        ("provenance=mock(실측 아님)", led.clone(), synthetic_attestation(vec![("provenance", sv("mock"))])),
        ("미지 schema_version", led.clone(), synthetic_attestation(vec![("schema_version", V::Int("99".into()))])),
        ("상속원이 VALIDATED 에 없음(체인 금지)", vec![entry((INHERIT_NGC, "0.99.0"), "self-test", "output/multi/resolved.json#upstream_delta")], synthetic_attestation(vec![("from_ref", sv("v0.99.0"))])),
        ("상속원 NGC 베이스 불일치", vec![entry(("26.03-py3", "0.22.1"), "self-test", "output/multi/resolved.json#upstream_delta")], synthetic_attestation(vec![("from_ref", sv("v0.22.1"))])),
        ("승인자 미기재(G1.6 미이행)", vec![entry((INHERIT_NGC, BASE_VLLM), "", "output/multi/resolved.json#upstream_delta")], synthetic_attestation(vec![])),
        ("attestation 포인터 형식 불량", vec![entry((INHERIT_NGC, BASE_VLLM), "self-test", "output/multi/resolved.json")], synthetic_attestation(vec![])),
    ];
    for (name, ledger, att) in &neg {
        let o = render_with(ledger, Some(att.clone()))?;
        req(o.contains("exit 1"), format!("음성대조 실패 — [{name}] 인데 상속이 통과:\n{o}"))?;
    }
    for e in INHERITED_SOURCE_BUILD_KEYS {
        req(!VALIDATED_SOURCE_BUILD_KEYS.contains(&e.key), format!("INHERITED 항목 {:?} 이 VALIDATED 에도 있음 — 두 주장이 겹치면 출처가 흐려진다", e.key))?;
        req(!e.attestation.is_empty() && !e.approved_by.is_empty() && !e.approved_kst.is_empty(), format!("INHERITED 항목 {:?} 필수 필드 결손", e.key))?;
        req(VALIDATED_SOURCE_BUILD_KEYS.contains(&e.inherits), format!("INHERITED 항목 {:?} 의 상속원 {:?} 이 VALIDATED 에 없음(체인 금지)", e.key, e.inherits))?;
    }
    println!(
        "[render] inherit self-test OK — 출구① 상속 양성2(NO_IMPACT·IMPACT∧위험0) + 음성{}건(N4 무증거·N5 UNDETERMINED 포함) 전부 fail-closed · 원장 항목 {}건 자기정합",
        2 + 2 + neg.len(),
        INHERITED_SOURCE_BUILD_KEYS.len()
    );
    Ok(())
}

fn read(p: &Path) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))
}

fn diff_msg(label: &str, rendered: &BTreeMap<String, String>, golden: &BTreeMap<String, String>) -> String {
    let only_r: Vec<_> = rendered.iter().filter(|(k, v)| golden.get(*k) != Some(v)).collect();
    let only_g: Vec<_> = golden.iter().filter(|(k, v)| rendered.get(*k) != Some(v)).collect();
    format!("{label} 렌더 != golden(집합 동치 위반)\n  rendered-side={only_r:?}\n  golden-side={only_g:?}")
}

fn is_class<T>(r: Result<T, Raised>, class: &str) -> bool {
    matches!(r, Err(e) if e.class == class)
}

fn envfile_tests(fixtures: &Path) -> Result<(BTreeMap<String, String>, BTreeMap<String, String>), String> {
    let golden = parse_env_pairs(&read(&fixtures.join("nccl_env_dgx-spark-gb10.golden"))?);
    let ic = |extra: Vec<(&str, V)>| {
        let items = vec![
            ("type", sv("RoCE v2")),
            ("hca_devices", lv(vec![sv("rocep1s0f1"), sv("roceP2p1s0f1")])),
            ("gid_index", V::Int("3".into())),
            ("socket_iface", sv("enp1s0f1np1")),
            ("bandwidth_gbps", V::Float(208.2)),
            ("platform_preset", sv("dgx-spark-gb10")),
        ];
        dv(vec![("interconnect", with_over(dv(items), extra))])
    };
    let man_ic = ic(vec![]);
    let rendered = parse_env_pairs(&render_nccl_envfile(&man_ic).map_err(raised)?);
    req(rendered == golden, diff_msg("NCCL", &rendered, &golden))?;
    req(rendered.len() == golden.len(), format!("NCCL 키 수는 golden에서 파생되어야 한다({} 기대, got {})", golden.len(), rendered.len()))?;
    req(is_class(build_nccl_env(&ic(vec![("platform_preset", sv("no-such-preset"))])), "KeyError"), "미지 platform_preset 인데 통과(fail-loud 위반)")?;
    req(is_class(build_nccl_env(&dv(vec![("interconnect", dv(vec![("platform_preset", sv("dgx-spark-gb10"))]))])), "ValueError"), "interconnect 필드 결손인데 통과(fail-loud 위반)")?;
    let rd = build_nccl_env(&ic(vec![("nccl_transport", sv("rdma"))])).map_err(raised)?;
    req(
        rd.get("NCCL_NET").map(String::as_str) == Some("IB") && rd.get("NCCL_IB_DISABLE").map(String::as_str) == Some("0") && rd.len() == golden.len(),
        format!("nccl_transport=rdma 렌더 이상: NCCL_NET={:?} IB_DISABLE={:?} keys={}", rd.get("NCCL_NET"), rd.get("NCCL_IB_DISABLE"), rd.len()),
    )?;
    req(build_nccl_env(&ic(vec![("nccl_transport", sv("socket"))])).map_err(raised)? == build_nccl_env(&man_ic).map_err(raised)?, "nccl_transport=socket 명시가 부재(기본)와 갈라졌다")?;
    req(is_class(build_nccl_env(&ic(vec![("nccl_transport", sv("roce"))])), "ValueError"), "미지 nccl_transport 인데 통과(fail-loud 위반)")?;
    println!("[render] NCCL self-test OK — .env.interconnect {}키 == golden 집합 동치 · 미지preset/결손 fail-loud 정상", golden.len());

    let cgolden = parse_env_pairs(&read(&fixtures.join("cluster_env_dgx-spark-gb10.golden"))?);
    let node = |role: &str, host: &str| dv(vec![("role", sv(role)), ("host", sv(host)), ("ssh_user", sv("deployer"))]);
    let man_nodes = dv(vec![("interconnect", dv(vec![("platform_preset", sv("dgx-spark-gb10"))])), ("nodes", lv(vec![node("main", "198.51.100.10"), node("sub", "198.51.100.11")]))]);
    let cr = parse_env_pairs(&render_cluster_envfile(&man_nodes).map_err(raised)?);
    req(cr == cgolden, diff_msg("cluster", &cr, &cgolden))?;
    req(cr.len() == cgolden.len(), format!("cluster {}키 기대(golden), got {}", cgolden.len(), cr.len()))?;
    let bad_preset = with_over(man_nodes.clone(), vec![("interconnect", dv(vec![("platform_preset", sv("no-such"))]))]);
    req(is_class(build_cluster_env(&bad_preset), "KeyError"), "미지 platform_preset 인데 통과(fail-loud 위반)")?;
    let one_node = dv(vec![("interconnect", dv(vec![("platform_preset", sv("dgx-spark-gb10"))])), ("nodes", lv(vec![node("main", "198.51.100.10")]))]);
    req(is_class(build_cluster_env(&one_node), "ValueError"), "sub 노드 결손인데 통과(fail-loud 위반)")?;
    println!("[render] cluster self-test OK — .env.cluster {}키 == golden 집합 동치 · 미지preset/노드결손 fail-loud 정상", cgolden.len());
    Ok((golden, cgolden))
}

fn tier_is(key: &str, tier: &str, detail: Option<&str>) -> R {
    let got = env_tier(key).map_err(raised)?;
    req(got.0 == tier && got.1.as_deref() == detail, format!("env_tier({key:?}) = {got:?}"))
}

fn env_tier_self_test(g: &BTreeMap<String, String>, c: &BTreeMap<String, String>) -> R {
    let keys: BTreeSet<&String> = g.keys().chain(c.keys()).collect();
    for key in &keys {
        let (tier, detail) = env_tier(key).map_err(raised)?;
        req(["env", "preset", "invariant"].contains(&tier), format!("env_tier({key:?}) = {tier:?} — golden 키가 규약 층 밖"))?;
        if tier == "env" {
            req(detail.as_deref().is_some_and(|d| !d.is_empty() && !d.starts_with("__")), format!("env_tier({key:?}) detail 이 manifest 필드가 아님: {detail:?}"))?;
        }
        if tier == "invariant" {
            req(detail.is_none(), format!("invariant 의 detail 은 None 이어야 한다: {key:?}={detail:?}"))?;
        }
    }
    tier_is("MASTER_HOST_IP", "env", Some("nodes[main].host"))?;
    tier_is("SLAVE_HOST_IP", "env", Some("nodes[sub].host"))?;
    tier_is("NCCL_SOCKET_IFNAME", "env", Some("interconnect.socket_iface"))?;
    tier_is("NCCL_IB_HCA", "env", Some("interconnect.hca_devices"))?;
    tier_is("RAY_PORT", "invariant", None)?;
    tier_is("NCCL_IB_DISABLE", "env", Some("interconnect.nccl_transport"))?;
    tier_is("NCCL_NET", "env", Some("interconnect.nccl_transport"))?;
    tier_is("NCCL_NET_GDR_LEVEL", "preset", Some("dgx-spark-gb10"))?;
    tier_is("MAX_JOBS", "preset", Some("dgx-spark-gb10"))?;
    let mut derived: BTreeSet<String> = BTreeSet::new();
    for k in &keys {
        if env_tier(k).map_err(raised)?.0 == "env" {
            derived.insert(k.to_string());
        }
    }
    let mut want: BTreeSet<String> = IFACE_ENV_KEYS.iter().map(|x| x.to_string()).collect();
    for k in ["NCCL_IB_HCA", "NCCL_IB_GID_INDEX", "MASTER_HOST_IP", "SLAVE_HOST_IP", "SSH_USER", "NCCL_NET", "NCCL_IB_DISABLE"] {
        want.insert(k.to_string());
    }
    req(derived == want, format!("① 환경값 키 집합이 렌더 함수와 갈라졌다: {derived:?}"))?;
    tier_is("NOT_A_RENDERED_KEY", "unknown", None)?;
    tier_is("CONFIG_FILE", "unknown", None)?;
    println!("[render] env_tier self-test OK — golden {}키 전수 층 판정 · ① {}키 탐침 파생 · 규약 밖 unknown(음성대조)", keys.len(), derived.len());
    Ok(())
}

const LEDGER_MARKER: &str = "RUN python3 - <<'PY'\nimport hashlib, json, os, platform, re, subprocess\n";

/// `_ledger_stanza_of`: (index of the RUN line, heredoc body, end index)
fn ledger_stanza_of(rendered: &str) -> Result<(usize, String, usize), String> {
    let idx = rendered.find(LEDGER_MARKER).ok_or("렌더 본문에 빌드 원장 스탠자가 없다")?;
    req(rendered[idx + 1..].find(LEDGER_MARKER).is_none(), "빌드 원장 스탠자가 두 번 이상 나온다")?;
    let body_start = idx + "RUN python3 - <<'PY'\n".len();
    let end = rendered[body_start..].find("\nPY\n").ok_or("원장 heredoc 종료 표지 부재")? + body_start;
    Ok((idx, rendered[body_start..end + 1].to_string(), end))
}

fn run_instructions(rendered: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Option<String> = None;
    for line in rendered.lines() {
        match cur.take() {
            None => {
                if line.starts_with("RUN ") && !line.contains("<<") {
                    let c = line[4..].to_string();
                    if !c.trim_end().ends_with('\\') {
                        out.push(c);
                    } else {
                        let t = c.trim_end();
                        cur = Some(t[..t.len() - 1].to_string());
                    }
                }
            }
            Some(mut c) => {
                let st = line.trim_end();
                if st.ends_with('\\') {
                    c.push(' ');
                    c.push_str(st[..st.len() - 1].trim());
                    cur = Some(c);
                } else {
                    out.push(format!("{c} {}", st.trim()));
                }
            }
        }
    }
    out
}

fn imports_vllm(body: &str) -> bool {
    body.lines().any(|l| {
        let t = l.trim_start();
        let after = |kw: &str| t.strip_prefix(kw).is_some_and(|r| r.starts_with(char::is_whitespace) && r.trim_start().starts_with("vllm"));
        after("import") || after("from")
    })
}

fn has_run_after(text: &str) -> bool {
    text.lines().any(|l| l.strip_prefix("RUN").is_some_and(|r| !r.starts_with(|c: char| c.is_alphanumeric() || c == '_')))
}

fn ledger_self_test(tdp: &Path, skill_root: &Path) -> R {
    let man = dv(vec![("cpu_arch", sv("aarch64")), ("nas_model_path", sv("/nas"))]);
    let res_src = res_of("0.26.0", "2.12.0", "26.05-py3", "13.2.0.046", "source-build", Some("12.1a"), Some(dv(vec![])));
    let res_whl = res_of("0.18.0", "2.10.0", "26.01-py3", "13.1", "wheel", None, Some(dv(vec![("cuda", sv("130")), ("manylinux", sv("manylinux_2_35"))])));
    let cases = [("Dockerfile.source-build.template", &res_src, "Dockerfile.source-build"), ("Dockerfile.template", &res_whl, "Dockerfile")];
    let tdir = skill_root.join("templates");
    for (tname, res, dname) in cases {
        let tpath = tdir.join(tname);
        let ttext = read(&tpath)?;
        let out = render(&tpath.to_string_lossy(), &man, res, false).map_err(raised)?;
        let (_idx, body, body_end) = ledger_stanza_of(&out)?;
        let args = ledger_build_args(&ttext);
        req(!args.is_empty(), format!("{tname}: 템플릿에 ARG 가 없다(파생 원천 부재)"))?;
        req(body.contains(&format!("ARGS = {}\n", json_list(&args))), format!("{tname}: 원장 ARGS 가 템플릿 ^ARG 줄과 다르다"))?;
        let rendered_args: BTreeSet<String> = ledger_build_args(&out).into_iter().collect();
        req(rendered_args == args.iter().cloned().collect::<BTreeSet<_>>(), format!("{tname}: 렌더 본문 ARG {rendered_args:?} ≠ 원장 {args:?}"))?;
        req(body.contains(&format!("DOCKERFILE = {}\n", json_string(dname))), format!("{tname}: 원장 dockerfile 이름이 템플릿 파생이 아니다"))?;
        req(!has_run_after(&out[body_end..]), format!("{tname}: 원장 스탠자 뒤에 RUN 이 있다(마지막이어야 한다)"))?;
        req(!imports_vllm(&body), format!("{tname}: 원장이 vllm 을 import 한다"))?;
    }
    eprintln!("[render] N/A: compile(ledger body) 와 원장 조립기 실행·분류 대조는 Python 인터프리터가 필요하다 — 이 실행체는 Python 을 띄우지 않는다(원장 본문은 이미지 안에서 도는 Python)");
    let probe_dir = tdp.join("probe");
    std::fs::create_dir_all(&probe_dir).map_err(|e| e.to_string())?;
    let ttext = read(&tdir.join("Dockerfile.source-build.template"))?;
    let probe = probe_dir.join("Dockerfile.source-build.template");
    std::fs::write(&probe, ttext.replacen("ARG BUILD_JOBS=16\n", "ARG BUILD_JOBS=16\nARG SELFTEST_NEW_ARG=0\n", 1)).map_err(|e| e.to_string())?;
    let (_, body2, _) = ledger_stanza_of(&render(&probe.to_string_lossy(), &man, &res_src, false).map_err(raised)?)?;
    req(body2.contains("\"SELFTEST_NEW_ARG\""), "새 ARG 가 원장 목록에 자동으로 들어가지 않았다(손목록 회귀)")?;
    println!("[render] build-ledger self-test OK(구조 대조) — ARG 파생(★새 ARG 자동)·마지막 RUN·vllm import 금지 · 분류기 실행 대조는 N/A(Python 필요)");
    loop_self_test(tdp, &tdir, &man, &res_src)
}

fn loop_self_test(tdp: &Path, tdir: &Path, man: &V, res_src: &V) -> R {
    let out = render(&tdir.join("Dockerfile.source-build.template").to_string_lossy(), man, res_src, false).map_err(raised)?;
    let runs = run_instructions(&out);
    let pre: Vec<&String> = runs.iter().filter(|r| r.contains("/tmp/build_patches_src/*.sh")).collect();
    let post: Vec<&String> = runs.iter().filter(|r| r.contains("/tmp/build_patches/*.sh")).collect();
    req(pre.len() == 1 && post.len() == 1, format!("pre/post 루프 RUN 을 하나씩 찾지 못했다: pre={} post={}", pre.len(), post.len()))?;
    let base = tdp.join("loop");
    let (src, pst, root) = (base.join("src"), base.join("post"), base.join("root"));
    std::fs::create_dir_all(&src).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&pst).map_err(|e| e.to_string())?;
    let lp = |cmd: &str| -> Result<(i32, String, String), String> {
        let cmd = cmd.replace("/tmp/build_patches_src", &src.to_string_lossy()).replace("/tmp/build_patches", &pst.to_string_lossy()).replace("/opt/easy-vllm", &root.to_string_lossy());
        let o = Command::new("sh").arg("-c").arg(cmd).output().map_err(|e| format!("POSIX sh 실행 불가 — 루프 실행 대조 불가(fail-loud): {e}"))?;
        Ok((o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned()))
    };
    let w = |p: PathBuf, t: &str| std::fs::write(p, t).map_err(|e| e.to_string());
    w(src.join(".gitkeep"), "")?;
    w(src.join("50-gated.sh"), "echo \"[50-gated] skip — SM12X_PORT=${SM12X_PORT:-0} (stock)\"\nexit 0\n")?;
    w(src.join("60-marked.sh"), "# model-trigger : Fixture4ForCausalLM\nprintf \"applied\\tfixture marker\\n\" > \"$EASY_VLLM_PATCH_STATUS\"\necho patched\n")?;
    w(pst.join("10-plain.sh"), "echo building\n")?;
    let (r1, o1, e1) = lp(pre[0])?;
    let (r2, _o2, e2) = lp(post[0])?;
    let tl = |t: &str| -> String { let c: Vec<char> = t.chars().collect(); c[c.len().saturating_sub(400)..].iter().collect() };
    req(r1 == 0 && r2 == 0, format!("루프 실행 실패: {} {}", tl(&e1), tl(&e2)))?;
    req(o1.contains("[build-patch-src] applying") && o1.contains("patched"), "패치 출력이 빌드 로그로 흐르지 않는다(tee 누락)")?;
    let d = root.join("ledger.d");
    for name in ["pre.inventory", "post.inventory", "pre.50-gated.sh.rc", "pre.50-gated.sh.sha256", "pre.50-gated.sh.log", "pre.60-marked.sh.status", "post.10-plain.sh.rc"] {
        req(d.join(name).is_file(), format!("루프가 원시 사실 {name} 을 남기지 않았다"))?;
    }
    let sha = read(&d.join("pre.50-gated.sh.sha256"))?;
    req(sha.trim().len() == 64, "스크립트 sha256 이 루프에서 캡처되지 않았다")?;
    w(src.join("70-broken.sh"), "echo boom\nexit 3\n")?;
    let (r3, o3, _) = lp(pre[0])?;
    req(r3 != 0 && o3.contains("FAIL 70-broken.sh rc=3"), format!("★ 실패한 패치가 빌드를 멈추지 않았다(rc={r3}): {}", { let c: Vec<char> = o3.chars().collect(); c[c.len().saturating_sub(300)..].iter().collect::<String>() }))?;
    println!("[render] build-ledger loop self-test OK(원시 캡처) — pre/post RUN 텍스트를 POSIX sh 로 실행 · 원시 캡처 · ★실패 패치 fail-loud 유지 · 원장 분류는 N/A(Python 필요)");
    Ok(())
}

fn materialize_self_test(tdp: &Path, skill_root: &Path) -> R {
    let td = tdp.join("mat");
    std::fs::create_dir_all(td.join("output/multi")).map_err(|e| e.to_string())?;
    let copied = materialize_configs(skill_root, &td, "multi").map_err(raised)?;
    req(copied.len() == RUNNER_SCRIPTS.len(), "materialize 복사 수")?;
    for name in RUNNER_SCRIPTS {
        let dst = td.join("output/multi/configs").join(name);
        req(dst.is_file(), format!("materialize 대상 부재: {name}"))?;
        let mode = std::fs::metadata(&dst).map_err(|e| e.to_string())?.permissions().mode() & 0o7777;
        req(mode == 0o755, format!("exact 0755 미보존: {name}"))?;
    }
    materialize_configs(skill_root, &td, "multi").map_err(raised)?;
    req(is_class(materialize_configs(&td.join("missing-shared-assets"), &td, "multi"), "FileNotFoundError"), "shared source 부재인데 통과(fail-loud 위반)")?;
    println!("[render] materialize self-test OK — 러너 스크립트 통로 복사(멱등·권한·fail-loud) 정상");
    Ok(())
}

fn body(scripts: &Path, tdp: &Path) -> R {
    let skill_root = scripts.parent().map(Path::to_path_buf).unwrap_or_else(|| scripts.to_path_buf());
    let tpl = "FROM nvcr.io/nvidia/pytorch:{{ NGC_TAG }}\n{{ SOURCE_BUILD_PATCH_GUARD }}\nARG VLLM_REF={{ VLLM_REF }}\n# arch={{ CPU_ARCH }} tag={{ IMAGE_TAG }} torch={{ TORCH_PIN }} cuarch={{ TORCH_CUDA_ARCH }}\n";
    let man = dv(vec![("cpu_arch", sv("aarch64")), ("nas_model_path", sv("/nas"))]);
    let sub = |res: &V| -> Result<String, String> { substitute(tpl, &build_context(&man, res, false).map_err(raised)?).map_err(raised) };
    let out_a = sub(&res_of("0.22.1", "2.11.0", "26.03-py3", "13.2.0.046", "source-build", Some("12.1a"), None))?;
    req(!out_a.contains("{{"), "leftover placeholder")?;
    req(out_a.contains("26.03-py3"), "ngc tag (a)")?;
    req(out_a.contains("0.22.1-cu132-aarch64-source"), format!("image tag (a): {out_a:?}"))?;
    req(out_a.contains("validated patch set"), "guard(validated) for (26.03, 0.22.1)")?;
    let out_b = sub(&res_of("0.23.0", "2.11.0", "26.05-py3", "13.2.0.046", "source-build", Some("12.1a"), None))?;
    req(out_b.contains("26.05-py3"), "ngc tag (b)")?;
    req(out_b.contains("0.23.0-cu132-aarch64-source"), format!("image tag (b): {out_b:?}"))?;
    req(out_b.contains("validated patch set"), "guard(validated) for (26.05, 0.23.0)")?;
    let out_bad = sub(&res_of("0.23.0", "2.11.0", "26.03-py3", "13.2.0.046", "source-build", Some("12.1a"), None))?;
    req(out_bad.contains("exit 1"), "guard(fail-loud) for (26.03, 0.23.0)")?;
    println!("[render] self-test OK — source-build 렌더 + (NGC베이스×vLLM버전) 키 가드(검증x2/fail-loud) 정상");
    inherit_self_test(tpl, &man)?;
    let (g, c) = envfile_tests(&skill_root.join("fixtures"))?;
    env_tier_self_test(&g, &c)?;
    ledger_self_test(tdp, &skill_root)?;
    materialize_self_test(tdp, &skill_root)
}

pub(crate) fn self_test(scripts: &Path) -> i32 {
    let tdp = std::env::temp_dir().join(format!("render-selftest-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    let res = std::fs::create_dir_all(&tdp).map_err(|e| e.to_string()).and_then(|_| body(scripts, &tdp));
    let _ = std::fs::remove_dir_all(&tdp);
    match res {
        Ok(()) => 0,
        Err(m) => {
            eprintln!("[render] self-test FAIL: {m}");
            1
        }
    }
}
