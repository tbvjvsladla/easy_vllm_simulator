//! The native `scan_node --self-test`: the regression harness of the original (`_self_test` of `scan_node.py`) over the Rust business of
//! this crate. Same cases, same check order, same stdout lines; no interpreter. Host facts (a loopback name, an unreadable interface) come
//! through [`Host`], the files of the cross-validate cases through [`Files`], the clock through [`Clock`].

use crate::manifest_gate::evaluate_contract;
use crate::scan_node::{
    assert_homogeneity, check_model_env, emit_gate, evaluate_gate, parse_gpu_memory_line, GateArgs,
};
use crate::scan_node_files::{cross_validate, Files};
use crate::scan_node_manifest::{build_sub_manifest, emit_manifest_block, Clock};
use crate::scan_node_probe::{iface_ipv4, resolves_to_loopback, Host};
use agent_runtime_support::pyjson::loads_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

fn j(text: &str) -> PyValue {
    loads_text(&PyStr::new(text)).expect("self-test fixture JSON")
}

fn g(value: &PyValue, key: &str) -> PyValue {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

fn eqs(value: &PyValue, text: &str) -> bool {
    *value == PyValue::str(text)
}

fn eqi(value: &PyValue, number: i64) -> bool {
    *value == PyValue::int(number)
}

fn py(flag: bool) -> &'static str {
    if flag {
        "True"
    } else {
        "False"
    }
}

fn shown(value: &PyValue) -> String {
    value.display_text().to_utf8().unwrap_or_default()
}

fn text_has(value: &PyValue, needle: &str) -> bool {
    shown(value).contains(needle)
}

fn len_of(value: &PyValue) -> usize {
    match value.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => items.len(),
        PyValue::Dict(map) => map.len(),
        _ => 0,
    }
}

fn gate_args(kwargs: &str) -> GateArgs {
    let k = j(kwargs);
    let or = |key: &str, default: PyValue| match k.peel() {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(default),
        _ => default,
    };
    GateArgs {
        declared: or("declared", PyValue::None),
        ic_present: or("ic_present", PyValue::None),
        peer_given: or("peer_given", PyValue::None),
        peer_reachable: or("peer_reachable", PyValue::None),
        bandwidth: or("bandwidth", PyValue::None),
        bw_floor: or("bw_floor", PyValue::None),
        per_port_gbps: or("per_port_gbps", PyValue::None),
        per_port_floor: or("per_port_floor", PyValue::None),
        branch: or("branch", PyValue::None),
        branch_topo: or("branch_topo", PyValue::None),
        mani_topo: or("mani_topo", PyValue::None),
        mani_path: or("mani_path", PyValue::str("manifest.yaml")),
        peer_ip: or("peer_ip", PyValue::None),
        egress_self: or("egress_self", PyValue::None),
        egress_peer: or("egress_peer", PyValue::None),
        model_env_ok: or("model_env_ok", PyValue::None),
        model_env_reason: or("model_env_reason", PyValue::None),
        manifest_read_error: or("manifest_read_error", PyValue::None),
    }
}

/// `(gate, exit code, consistent)` of one `evaluate_gate` call, `None` when the function raised.
fn run_gate(kwargs: &str) -> Option<(String, i64, bool)> {
    let verdict = evaluate_gate(&gate_args(kwargs)).ok()?;
    let items = match verdict.peel() {
        PyValue::Tuple(items) | PyValue::List(items) => items.clone(),
        _ => return None,
    };
    let code = match items.get(2)?.peel() {
        PyValue::Int(number) => number.to_string().parse::<i64>().ok()?,
        _ => return None,
    };
    Some((
        shown(&g(items.get(1)?, "branch")),
        code,
        g(items.first()?, "consistent").is_truthy(),
    ))
}

struct Tally {
    out: String,
    passed: usize,
    n: usize,
}

impl Tally {
    fn line(&mut self, ok: bool, body: &str) {
        self.out.push_str(&format!(
            "  [{}] {body}\n",
            if ok { "PASS" } else { "FAIL" }
        ));
        self.passed += usize::from(ok);
        self.n += 1;
    }
}

fn gate_rows(tally: &mut Tally, label: &str, rows: &[(&str, String, (&str, i64, bool))]) {
    for (name, kwargs, (eg, ec, econ)) in rows {
        let got = run_gate(kwargs);
        let ok = got
            .as_ref()
            .is_some_and(|(gate, code, con)| gate == eg && code == ec && con == econ);
        let (gate, code, con) = got.unwrap_or((String::from("<raised>"), -1, false));
        let tail = if ok {
            String::new()
        } else {
            format!("  ← 기대 ({eg},{ec},{})", py(*econ))
        };
        let body = if label.is_empty() {
            format!("{name}: gate={gate} exit={code} consistent={}{tail}", py(con))
        } else {
            format!("{label}:{name}: gate={gate} exit={code}{tail}")
        };
        tally.line(ok, &body);
    }
}

fn gate_kwargs(extra: &str) -> String {
    format!("{{\"bw_floor\":180.0,{extra}}}")
}

/// Runs the whole self-test; returns the stdout text and the exit status.
pub fn run(host: &dyn Host, files: &dyn Files, clock: &dyn Clock) -> (String, i32) {
    let mut t = Tally {
        out: String::new(),
        passed: 0,
        n: 0,
    };
    let multi = |extra: &str| {
        gate_kwargs(&format!(
            "\"declared\":\"multi\",\"branch_topo\":\"multi\",\"mani_topo\":\"multi\",{extra}"
        ))
    };
    let single = |extra: &str| {
        gate_kwargs(&format!(
            "\"declared\":\"single\",\"peer_given\":false,\"peer_reachable\":null,\"bandwidth\":null,\"branch_topo\":\"single\",\"mani_topo\":\"single\",{extra}"
        ))
    };
    let rows: Vec<(&str, String, (&str, i64, bool))> = vec![
        ("multi-ready(구조+성능)", multi("\"ic_present\":true,\"peer_given\":true,\"peer_reachable\":true,\"bandwidth\":208.2"), ("multi-ready", 0, true)),
        ("multi γ(성능 fail-closed)", multi("\"ic_present\":true,\"peer_given\":true,\"peer_reachable\":true,\"bandwidth\":100.0"), ("gamma", 2, true)),
        ("multi γ(peer 미도달)", multi("\"ic_present\":true,\"peer_given\":true,\"peer_reachable\":false,\"bandwidth\":208.2,\"peer_ip\":\"x\""), ("gamma", 2, true)),
        ("multi γ(no RoCE)", multi("\"ic_present\":false,\"peer_given\":true,\"peer_reachable\":true,\"bandwidth\":208.2"), ("gamma", 2, false)),
        ("multi pending-perf(미측정=fail-closed)", multi("\"ic_present\":true,\"peer_given\":true,\"peer_reachable\":true,\"bandwidth\":null"), ("multi-ready-candidate", 2, true)),
        ("multi 포트당 미달(합산은 통과)", multi("\"ic_present\":true,\"peer_given\":true,\"peer_reachable\":true,\"bandwidth\":200.0,\"per_port_gbps\":60.0,\"per_port_floor\":100.0"), ("gamma", 2, true)),
        ("multi 포트당·합산 모두 통과", multi("\"ic_present\":true,\"peer_given\":true,\"peer_reachable\":true,\"bandwidth\":208.0,\"per_port_gbps\":104.0,\"per_port_floor\":100.0"), ("multi-ready", 0, true)),
        ("single α", single("\"ic_present\":false"), ("alpha", 0, true)),
        ("single α + RoCE(경고 비blocking)", single("\"ic_present\":true"), ("alpha", 0, true)),
        ("single on multi-branch(혼재차단)", gate_kwargs("\"declared\":\"single\",\"ic_present\":true,\"peer_given\":false,\"peer_reachable\":null,\"bandwidth\":null,\"branch_topo\":\"multi\",\"mani_topo\":\"multi\""), ("alpha", 2, false)),
        ("manifest≠branch(혼재차단)", gate_kwargs("\"declared\":\"multi\",\"ic_present\":true,\"peer_given\":true,\"peer_reachable\":true,\"bandwidth\":208.2,\"branch_topo\":\"multi\",\"mani_topo\":\"single\""), ("gamma", 2, false)),
        ("★declared≠manifest 직접모순(③)", gate_kwargs("\"declared\":\"single\",\"ic_present\":false,\"peer_given\":false,\"peer_reachable\":null,\"bandwidth\":null,\"branch\":\"single-node\",\"branch_topo\":\"single\",\"mani_topo\":\"multi\""), ("alpha", 2, false)),
        ("★branch 미해소 → 3자일치 판정불가(③)", gate_kwargs("\"declared\":\"single\",\"ic_present\":false,\"peer_given\":false,\"peer_reachable\":null,\"bandwidth\":null,\"branch\":\"feature/x\",\"branch_topo\":null,\"mani_topo\":\"single\""), ("alpha", 2, false)),
    ];
    gate_rows(&mut t, "", &rows);

    for (name, emit, topo, expect) in [
        ("emit+auto → 거부(fail-closed)", true, "auto", 3),
        ("emit+single → 통과", true, "single", 0),
        ("emit+multi → 통과", true, "multi", 0),
        ("no-emit+auto → 통과(보고만)", false, "auto", 0),
    ] {
        let got = emit_gate(&PyValue::Bool(emit), &PyValue::str(topo));
        let ok = got == expect;
        let tail = if ok { String::new() } else { format!("  ← 기대 {expect}") };
        t.line(ok, &format!("{name}: emit_gate={got}{tail}"));
    }

    let emit_cases: Vec<(&str, &str, Vec<&str>)> = vec![
        ("single GPU-less(전필드 None)", r#"{"topology_declared":"single","cpu_arch":"x86_64","cuda_version":null,"gpus_per_node":null,"nodes":[{"role":"main","host":"node-a","hostname":"node-a","ssh_user":"u","work_dir":"/w"}],"interconnect":{"type":"generic-ethernet","hca_devices":[],"gid_index":null,"socket_iface":null,"bandwidth_gbps":null,"platform_preset":null}}"#, vec!["main"]),
        ("multi RoCE(채워짐)", r#"{"topology_declared":"multi","cpu_arch":"aarch64","cuda_version":"132","gpus_per_node":1,"gpu_model":"NVIDIA GB10","model_source":"managed","nas_model_path":"/srv/fixture-nas/models","nodes":[{"role":"main","node_id":"main","host":"node-a","hostname":"node-a","ssh_user":"u","work_dir":"/w"},{"role":"sub","node_id":"sub","host":"node-b","hostname":"node-b","ssh_user":"u","work_dir":"/w"}],"homogeneity":{"verified":true,"peer":{"gpu_model":"NVIDIA GB10","driver":"580.173.02","cuda":"13.2"}},"interconnect":{"type":"RoCE v2","hca_devices":["mlx5_0"],"gid_index":3,"mtu":9000,"socket_iface":"enp1s0f0","bandwidth_gbps":208.2,"platform_preset":"dgx-spark-gb10"}}"#, vec!["main", "sub"]),
        ("multi 서브 미상(센티넬 남김)", r#"{"topology_declared":"multi","cpu_arch":"aarch64","cuda_version":"132","gpus_per_node":1,"gpu_model":"NVIDIA GB10","nodes":[{"role":"main","host":"node-a","hostname":"node-a","ssh_user":"u","work_dir":"/w"},{"role":"sub","host":"node-b"}],"interconnect":{"type":"RoCE v2","hca_devices":[],"gid_index":null,"socket_iface":null,"bandwidth_gbps":null,"platform_preset":null}}"#, vec!["main", "sub"]),
    ];
    for (name, result, roles) in emit_cases {
        let blk = emit_manifest_block(&j(result), clock)
            .map(|v| shown(&v))
            .unwrap_or_default();
        let no_none = !blk.contains("None");
        let nodes_ok = blk.contains("nodes:") && roles.iter().all(|r| blk.contains(&format!("role: {r}")));
        let attest_ok = blk.contains("terraforming:") && blk.contains("complete: true") && blk.contains("branch_verified: true");
        let preset_ok = (blk.contains("platform_preset: dgx-spark-gb10") || blk.contains("platform_preset: generic"))
            && !blk.contains("platform_preset: null")
            && blk.contains("platform_preset_source:");
        let keys_ok = ["model_source:", "nas_model_path:", "ssh_user:", "work_dir:", "self_role: main"]
            .iter()
            .all(|k| blk.contains(k));
        let has_sentinel = blk
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .any(|l| l.contains("__REQUIRED__"));
        let sentinel_ok = if name.starts_with("multi RoCE") {
            !has_sentinel && blk.contains("hw_verified: true")
        } else if name.starts_with("multi 서브 미상") {
            has_sentinel && blk.contains("hw_verified: false")
        } else {
            has_sentinel
        };
        let ok = no_none && nodes_ok && blk.contains("topology:") && attest_ok && preset_ok && keys_ok && sentinel_ok;
        t.line(ok, &format!(
            "emit:{name}: no-None={} nodes={} attest={} preset={} keys={} sentinel={}",
            py(no_none), py(nodes_ok), py(attest_ok), py(preset_ok), py(keys_ok), py(sentinel_ok)
        ));
    }

    let hl = r#"{"cpu_arch":"aarch64","gpu_model":"NVIDIA GB10","gpus_per_node":1,"cuda":"13.2","driver":"565.57.01"}"#;
    let with = |key: &str, value: &str| {
        let mut base = j(hl);
        if let PyValue::Dict(map) = &mut base {
            map.set(key, j(value));
        }
        base
    };
    let homo_cases: Vec<(&str, PyValue, bool)> = vec![
        ("동질 pass", j(hl), true),
        ("arch 불일치 → block", with("cpu_arch", "\"x86_64\""), false),
        ("gpu_model 불일치 → block", with("gpu_model", "\"NVIDIA RTX 5090\""), false),
        ("gpus_per_node 불일치 → block", with("gpus_per_node", "2"), false),
        ("cuda minor drift → warn-pass", with("cuda", "\"13.1\""), true),
        ("cuda major 불일치 → block", with("cuda", "\"12.9\""), false),
        ("driver minor drift → warn-pass", with("driver", "\"565.90.07\""), true),
        ("gpu_model 미상(None) → block(WARN-4)", with("gpu_model", "null"), false),
        ("peer 수집실패 → block", j("{}"), false),
    ];
    for (name, peer, want) in homo_cases {
        let h = assert_homogeneity(&j(hl), &peer).unwrap_or(PyValue::None);
        let verified = g(&h, "verified").is_truthy();
        let (blocks, warns) = (len_of(&g(&h, "blocks")), len_of(&g(&h, "warns")));
        let mut ok = verified == want;
        if !want {
            ok = ok && blocks >= 1;
        }
        if name.contains("warn-pass") {
            ok = ok && blocks == 0 && warns >= 1;
        }
        t.line(ok, &format!("homo:{name}: verified={} blocks={blocks} warns={warns}", py(verified)));
    }

    let s = |x: &str| PyValue::str(x);
    let cme: Vec<(&str, [PyValue; 4], bool)> = vec![
        ("managed+NAS도달 → ok", [s("managed"), s("online"), PyValue::Bool(true), PyValue::None], true),
        ("managed+NAS미도달 → block", [s("managed"), s("online"), PyValue::Bool(false), PyValue::None], false),
        ("managed+NAS미측정(None) → ok(정보부족≠실패)", [s("managed"), s("online"), PyValue::None, PyValue::None], true),
        ("ephemeral+egress-restricted+토큰부재 → block", [s("ephemeral"), s("restricted"), PyValue::None, PyValue::Bool(false)], false),
        ("ephemeral+egress-restricted+토큰존재 → ok", [s("ephemeral"), s("restricted"), PyValue::None, PyValue::Bool(true)], true),
        ("ephemeral+egress-online+토큰부재 → ok(온라인이라 무관)", [s("ephemeral"), s("online"), PyValue::None, PyValue::Bool(false)], true),
        ("custom+egress-restricted+토큰부재 → block", [s("custom"), s("restricted"), PyValue::None, PyValue::Bool(false)], false),
    ];
    for (name, a, want) in cme {
        let r = check_model_env(&a[0], &a[1], &a[2], &a[3]);
        let got = g(&r, "ok").is_truthy();
        let ok = got == want;
        let tail = if ok { String::new() } else { format!("  ← 기대 {}", py(want)) };
        t.line(ok, &format!("model_env:{name}: ok={}{tail}", py(got)));
    }

    let ge: Vec<(&str, String, (&str, i64, bool))> = vec![
        ("egress-restricted+model-ok → pass(egress 는 attestation 뿐)", single("\"ic_present\":false,\"egress_self\":\"online\",\"egress_peer\":\"restricted\",\"model_env_ok\":true"), ("alpha", 0, true)),
        ("model-env-missing → block(fail-closed)", single("\"ic_present\":false,\"model_env_ok\":false,\"model_env_reason\":\"ephemeral: HF_TOKEN 부재 ∧ egress-restricted\""), ("alpha", 2, true)),
        ("multi+model-env-missing → gamma block", multi("\"ic_present\":true,\"peer_given\":true,\"peer_reachable\":true,\"bandwidth\":208.2,\"model_env_ok\":false,\"model_env_reason\":\"managed: NAS 미도달\""), ("gamma", 2, true)),
    ];
    gate_rows(&mut t, "gate-egress", &ge);

    // cross_validate source merge, over a private temporary directory
    let dir = std::env::temp_dir().join(format!("scan-node-selftest-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let at = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let (comp, envf, none) = (at("docker-compose.yaml"), at(".env.interconnect"), at("없음"));
    let ic = j(r#"{"hca_devices":["roceP2p1s0f1","rocep1s0f1"],"gid_index":3,"socket_iface":"enp1s0f1np1"}"#);
    let write = |path: &str, body: &str| {
        let _ = std::fs::write(path, body);
    };
    let cv = |c: &str, e: &str| cross_validate(&ic, &PyValue::str(c), &PyValue::str(e), files).unwrap_or(PyValue::None);
    write(&comp, "services:\n  vllm:\n    environment:\n      NCCL_IB_HCA: \"=rocep1s0f1,roceP2p1s0f1\"\n      NCCL_SOCKET_IFNAME: enp1s0f1np1\n");
    write(&envf, "# rendered\nNCCL_IB_GID_INDEX=3\nNCCL_IB_HCA==rocep1s0f1,roceP2p1s0f1\nNCCL_SOCKET_IFNAME=enp1s0f1np1\n");
    let r = cv(&comp, &none);
    let c1 = g(&r, "checked").is_truthy() && g(&r, "all_match").is_truthy() && eqs(&g(&g(&r, "sources"), "NCCL_IB_HCA"), "compose");
    write(&comp, "services:\n  vllm:\n    env_file:\n      - envs/.env.interconnect\n");
    let r = cv(&comp, &envf);
    let c2 = g(&r, "checked").is_truthy() && g(&r, "all_match").is_truthy() && len_of(&g(&r, "checks")) == 3
        && eqs(&g(&g(&r, "sources"), "NCCL_IB_GID_INDEX"), "env.interconnect");
    let r = cv(&comp, &none);
    let c3 = !g(&r, "checked").is_truthy() && text_has(&g(&r, "reason"), "NCCL env 부재");
    write(&envf, "NCCL_SOCKET_IFNAME=wrong0\n");
    let r = cv(&comp, &envf);
    let c4 = g(&r, "checked").is_truthy() && !g(&r, "all_match").is_truthy();
    let _ = std::fs::remove_dir_all(&dir);
    for (name, ok) in [
        ("compose-inline-only → checked+all_match", c1),
        ("env.interconnect-only → checked+all_match(침묵 no-op 아님)", c2),
        ("양쪽 부재 → checked:false + 명시 reason", c3),
        ("env 값 불일치 → all_match False", c4),
    ] {
        t.line(ok, &format!("cross-validate:{name}"));
    }

    let none_of = |r: crate::scan_node::Res<PyValue>| matches!(r.map(|v| v.peel().clone()), Ok(PyValue::None));
    let loop_name = resolves_to_loopback(host, &s("localhost")).unwrap_or(PyValue::None);
    let loop_text = if loop_name.is_truthy() { shown(&loop_name) } else { String::new() };
    for (name, ok) in [
        ("iface 미지정 → None", none_of(iface_ipv4(host, &PyValue::None))),
        ("없는 iface → None(예외 ✗)", none_of(iface_ipv4(host, &s("nonexistent-iface-xyz0")))),
        ("localhost → 루프백 탐지", loop_text.starts_with("127.")),
        ("빈 이름 → None", none_of(resolves_to_loopback(host, &s("")))),
        ("해소 불가 이름 → None(예외 ✗)", none_of(resolves_to_loopback(host, &s("no-such-host.invalid")))),
    ] {
        t.line(ok, name);
    }

    let fx_result = r#"{"declared_topology":"single","model_source":"managed","nas_model_path":"/srv/test-models","homogeneity":{"verified":true,"verified_rule":"single:peer-observed(gpus>=1)"},"nodes":[{"role":"main","host":"203.0.113.10"},{"role":"sub","host":"203.0.113.11","hw_verified":true}]}"#;
    let fx_peer = r#"{"cpu_arch":"aarch64","gpu_model":"NVIDIA TEST","gpus_per_node":1,"cuda":"13.2","driver":"580.0.1"}"#;
    let fx_main = r#"{"quant_model_path":"/srv/quant","tiktoken_host_path":"/srv/tok","origin_url":"https://example.invalid/r","interconnect":{"type":"RoCE v2","mtu":9000}}"#;
    let patched = |base: &str, key: &str, value: PyValue| {
        let mut v = j(base);
        if let PyValue::Dict(map) = &mut v {
            map.set(key, value);
        }
        v
    };
    let flag_of = |sub: crate::scan_node::Res<PyValue>| {
        sub.ok()
            .and_then(|m| evaluate_contract(&m, "single").ok())
            .is_some_and(|e| e.flag())
    };
    let build = |r: &PyValue, p: &PyValue, scanned: PyValue, egress: PyValue| {
        build_sub_manifest(r, p, &j(fx_main), &scanned, &egress, clock)
    };
    let sm = build(&j(fx_result), &j(fx_peer), s("2026090516"), s("online")).unwrap_or(PyValue::None);
    let flag = sm_flag(&sm);
    let roles: Vec<String> = match g(&sm, "nodes").peel() {
        PyValue::List(items) => items.iter().map(|n| shown(&g(n, "role"))).collect(),
        _ => Vec::new(),
    };
    let net = g(&sm, "network");
    let sc: Vec<(&str, bool)> = vec![
        ("sub-manifest: Flag 계약 통과(issued_by=main)", flag && eqs(&g(&g(&sm, "terraforming"), "issued_by"), "main")),
        ("sub-manifest: self_role=sub · topology=single", eqs(&g(&sm, "self_role"), "sub") && eqs(&g(&sm, "topology"), "single")),
        ("sub-manifest: cuda 13.2→'132' · gpus int · driver 실측", eqs(&g(&sm, "cuda_version"), "132") && eqi(&g(&sm, "gpus_per_node"), 1) && eqs(&g(&sm, "driver_version"), "580.0.1")),
        ("sub-manifest: 경로는 공유 NAS(메인 manifest) · egress 는 서브 실측 출처", eqs(&g(&sm, "quant_model_path"), "/srv/quant") && len_of(&net) == 2 && eqs(&g(&net, "egress"), "online") && eqs(&g(&net, "egress_source"), "measured:peer-probe")),
        ("sub-manifest: 로스터 = 메인과 동일(main+sub)", roles == ["main", "sub"]),
        ("sub-manifest 음성대조: model_source 없음 → Flag 무효", !flag_of(build(&patched(fx_result, "model_source", PyValue::None), &j(fx_peer), PyValue::None, PyValue::None))),
        ("sub-manifest 음성대조: 서브 GPU 미관측 → Flag 무효(gpus 결손)", !flag_of(build(&j(fx_result), &patched(fx_peer, "gpus_per_node", PyValue::None), PyValue::None, PyValue::None))),
    ];
    for (name, ok) in sc {
        t.line(ok, name);
    }

    let gm = |line: &str| parse_gpu_memory_line(&s(line)).unwrap_or(PyValue::None);
    let triple = |v: &PyValue, m: Option<&str>, vram: Option<i64>, cc: Option<&str>| {
        let part = |value: PyValue, want: Option<PyValue>| value == want.unwrap_or(PyValue::None);
        part(g(v, "gpu_memory_model"), m.map(PyValue::str))
            && part(g(v, "gpu_vram_mib"), vram.map(PyValue::int))
            && part(g(v, "compute_capability"), cc.map(PyValue::str))
    };
    for (name, ok) in [
        ("GB10 '[N/A], 12.1' → unified · vram None · cc 12.1", triple(&gm("[N/A], 12.1"), Some("unified"), None, Some("12.1"))),
        ("discrete '32607, 12.0' → discrete · 32607 MiB · cc 12.0", triple(&gm("32607, 12.0"), Some("discrete"), Some(32607), Some("12.0"))),
        ("nvidia-smi 부재(빈 줄) → 전부 None(추측 ✗)", triple(&gm(""), None, None, None)),
        ("판독 불가 값 → None(unified 로 접지 않는다)", g(&gm("garbage, x"), "gpu_memory_model") == PyValue::None),
    ] {
        t.line(ok, name);
    }

    let all = t.passed == t.n;
    t.out.push_str(&format!(
        "self-test: {}/{} {}\n",
        t.passed,
        t.n,
        if all { "PASS" } else { "FAIL" }
    ));
    (t.out, if all { 0 } else { 1 })
}

fn sm_flag(sm: &PyValue) -> bool {
    evaluate_contract(sm, "single").is_ok_and(|e| e.flag())
}
