//! `render_dockerfile.py`, unit A: the manifest-driven env files (`--nccl-envfile`, `--cluster-envfile`), the delivery-lane materializers
//! (`--materialize-configs`, `--materialize-env`) and the env-key tier judgement (`env_tier`, exposed here as the new `--env-tier KEY`;
//! the original has no such command line option — it is an in-process function for `slave_forward`).
//!
//! Unit B (`--canonical-kind`, `--template`/`--resolved`: the template render with the source-build guard, the build-ledger stanza and the
//! Flag gate) lives in `render_template`. `--self-test` is `render_template::selftest`.
//! An exception the original leaves uncaught (a Python traceback, status 1) is reported as `<Class>: <message>` on stderr with status 1;
//! the traceback frames are not reproduced.

use crate::gate_cli::{self, Values};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use agent_runtime_support::pyyaml::safe_load;
use recipe_runtime::cliparse::{Kind, Opt, Spec};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub const PROGRAM: &str = "render_dockerfile.py";
pub const RUNNER_SCRIPTS: [&str; 3] = ["serve_runner.sh", "debug-init.sh", "arm_patch.sh"];

type Env = BTreeMap<String, String>;

fn env_of(pairs: &[(&str, &str)]) -> Env {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn nccl_presets() -> Vec<(&'static str, Env)> {
    vec![
        (
            "dgx-spark-gb10",
            env_of(&[
                ("NCCL_IB_MERGE_NICS", "1"),
                ("NCCL_IB_QPS_PER_CONNECTION", "4"),
                ("NCCL_IB_SPLIT_DATA_ON_QPS", "0"),
                ("NCCL_NET_GDR_LEVEL", "LOC"),
                ("NCCL_NET_GDR_C2C", "0"),
                ("NCCL_NET_GDR_READ", "0"),
                ("NCCL_DMABUF_ENABLE", "0"),
                ("NCCL_CROSS_NIC", "1"),
            ]),
        ),
        ("generic", Env::new()),
    ]
}

fn nccl_invariants() -> Env {
    env_of(&[("NCCL_DEBUG", "INFO"), ("NCCL_DEBUG_SUBSYS", "INIT,NET,GRAPH,ENV")])
}

fn nccl_transports() -> Vec<(&'static str, Env)> {
    vec![
        ("socket", env_of(&[("NCCL_IB_DISABLE", "1"), ("NCCL_NET", "Socket")])),
        ("rdma", env_of(&[("NCCL_IB_DISABLE", "0"), ("NCCL_NET", "IB")])),
    ]
}

pub(crate) const IFACE_ENV_KEYS: [&str; 5] = [
    "NCCL_SOCKET_IFNAME",
    "GLOO_SOCKET_IFNAME",
    "TP_SOCKET_IFNAME",
    "UCX_NET_DEVICES",
    "OMPI_MCA_btl_tcp_if_include",
];

fn cluster_presets() -> Vec<(&'static str, Env)> {
    vec![
        (
            "dgx-spark-gb10",
            env_of(&[
                ("RAY_memory_usage_threshold", "0.99"),
                ("RAY_memory_monitor_refresh_ms", "0"),
                ("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True"),
                ("RAY_OBJECT_STORE_MEMORY", "2000000000"),
                ("MAX_JOBS", "4"),
            ]),
        ),
        ("generic", Env::new()),
    ]
}

fn cluster_invariants() -> Env {
    env_of(&[("RAY_PORT", "6379")])
}

// ── a manifest value, with Python's `str()` and truthiness ────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum V {
    None,
    Bool(bool),
    Int(String),
    Float(f64),
    Str(String),
    List(Vec<V>),
    Dict(Vec<(String, V)>),
}

impl V {
    pub(crate) fn from_py(value: &PyValue) -> V {
        match value.peel() {
            PyValue::None => V::None,
            PyValue::Bool(b) => V::Bool(*b),
            PyValue::Int(n) => V::Int(n.to_string()),
            PyValue::Float(f) => V::Float(*f),
            PyValue::Str(s) => V::Str(s.to_utf8().unwrap_or_else(|| String::from_utf8_lossy(&s.to_backslashreplace_bytes()).into_owned())),
            PyValue::List(items) | PyValue::Tuple(items) => V::List(items.iter().map(V::from_py).collect()),
            PyValue::Dict(dict) => V::Dict(
                dict.iter()
                    .map(|(k, v)| (k.to_utf8().unwrap_or_default(), V::from_py(v)))
                    .collect(),
            ),
            other => V::Str(other.repr()),
        }
    }

    pub(crate) fn truthy(&self) -> bool {
        match self {
            V::None => false,
            V::Bool(b) => *b,
            V::Int(n) => n != "0",
            V::Float(f) => *f != 0.0,
            V::Str(s) => !s.is_empty(),
            V::List(l) => !l.is_empty(),
            V::Dict(d) => !d.is_empty(),
        }
    }

    /// Python `str(value)`.
    pub(crate) fn py_str(&self) -> String {
        match self {
            V::None => "None".into(),
            V::Bool(true) => "True".into(),
            V::Bool(false) => "False".into(),
            V::Int(n) => n.clone(),
            V::Float(f) => format!("{f:?}"),
            V::Str(s) => s.clone(),
            V::List(l) => format!("[{}]", l.iter().map(V::py_repr).collect::<Vec<_>>().join(", ")),
            V::Dict(d) => format!("{{{}}}", d.iter().map(|(k, v)| format!("{}: {}", py_repr_str(k), v.py_repr())).collect::<Vec<_>>().join(", ")),
        }
    }

    pub(crate) fn py_repr(&self) -> String {
        match self {
            V::Str(s) => py_repr_str(s),
            other => other.py_str(),
        }
    }

    pub(crate) fn get(&self, key: &str) -> Option<&V> {
        match self {
            V::Dict(d) => d.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

pub(crate) fn py_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

fn py_repr_list(items: &[&str]) -> String {
    format!("[{}]", items.iter().map(|s| py_repr_str(s)).collect::<Vec<_>>().join(", "))
}

/// An uncaught Python exception of the original: class and message.
pub struct Raised {
    pub class: &'static str,
    pub message: String,
}

pub(crate) fn value_error<T>(message: String) -> Result<T, Raised> {
    Err(Raised { class: "ValueError", message })
}

pub(crate) fn key_error<T>(message: String) -> Result<T, Raised> {
    // `str(KeyError(msg))` is `repr(msg)`
    Err(Raised { class: "KeyError", message: py_repr_str(&message) })
}

/// `manifest.get(key, default)` then `str(...)`.
fn get_str(manifest: &V, key: &str, default: &str) -> String {
    manifest.get(key).map_or_else(|| default.to_string(), V::py_str)
}

fn py_strip(s: &str) -> &str {
    s.trim()
}

fn load_manifest(path: &str) -> Result<V, Raised> {
    let bytes = std::fs::read(path).map_err(|e| Raised {
        class: if e.kind() == std::io::ErrorKind::NotFound { "FileNotFoundError" } else { "OSError" },
        message: if e.kind() == std::io::ErrorKind::NotFound {
            format!("[Errno 2] No such file or directory: {}", py_repr_str(path))
        } else {
            e.to_string()
        },
    })?;
    match safe_load(&bytes, &PyStr::new(path)) {
        Ok(value) => {
            let v = V::from_py(&value);
            if !v.truthy() {
                return Ok(V::Dict(Vec::new()));
            }
            Ok(v)
        }
        Err(error) => Err(Raised { class: error.class, message: error.message.to_utf8().unwrap_or_default() }),
    }
}

// ── env builders ───────────────────────────────────────────────────────────────────────────────────

fn lookup<'a>(table: &'a [(&'static str, Env)], key: &str) -> Option<&'a Env> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
}

fn sorted_names(table: &[(&'static str, Env)]) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = table.iter().map(|(k, _)| *k).collect();
    names.sort_unstable();
    names
}

pub fn build_nccl_env(manifest: &V) -> Result<Env, Raised> {
    let ic = match manifest.get("interconnect") {
        Some(v) if v.truthy() => v,
        _ => return value_error("manifest.interconnect 부재 — NCCL 렌더 불가(fail-loud, 무증거 진행 금지)".into()),
    };
    let hca = ic.get("hca_devices");
    let gid = ic.get("gid_index");
    let iface = ic.get("socket_iface");
    for (name, v) in [("hca_devices", hca), ("gid_index", gid), ("socket_iface", iface)] {
        let missing = match v {
            None | Some(V::None) => true,
            Some(V::Str(s)) => s.is_empty(),
            Some(V::List(l)) => l.is_empty(),
            _ => false,
        };
        if missing {
            return value_error(format!("manifest.interconnect.{name} 결손 — fail-loud"));
        }
    }
    let hca_list: Vec<String> = match hca {
        Some(V::Str(s)) => s.split(',').map(|h| py_strip(h).to_string()).filter(|h| !h.is_empty()).collect(),
        Some(V::List(l)) => l.iter().map(V::py_str).collect(),
        Some(other) => {
            return Err(Raised { class: "AttributeError", message: format!("'{}' object has no attribute 'split'", type_name(other)) })
        }
        None => Vec::new(),
    };
    let gid = gid.map(V::py_str).unwrap_or_default();
    let iface = iface.map(V::py_str).unwrap_or_default();

    let mut env = Env::new();
    env.insert("NCCL_IB_HCA".into(), format!("={}", hca_list.join(",")));
    env.insert("NCCL_IB_GID_INDEX".into(), gid);
    for k in IFACE_ENV_KEYS {
        env.insert(k.into(), iface.clone());
    }
    let presets = nccl_presets();
    let preset_key = ic.get("platform_preset").map(V::py_str);
    // `None` is never a key of the table
    let preset = match &preset_key {
        Some(k) if !matches!(ic.get("platform_preset"), Some(V::None)) => lookup(&presets, k),
        _ => None,
    };
    let Some(preset) = preset else {
        let shown = match ic.get("platform_preset") {
            None => "None".to_string(),
            Some(v) => v.py_str(),
        };
        return key_error(format!(
            "platform_preset '{shown}' 미정의 — 알려진 {} (fail-loud·오타 방지; 미검증 플랫폼은 'generic'(튜닝 0)으로 일단 기동→comms 스모크 중재, 검증 후 전용 프리셋으로 승격)",
            py_repr_list(&sorted_names(&presets))
        ));
    };
    env.extend(preset.clone());
    env.extend(nccl_invariants());
    let transport = match ic.get("nccl_transport") {
        Some(v) if v.truthy() => v.py_str(),
        _ => "socket".to_string(),
    };
    let transports = nccl_transports();
    let Some(t) = lookup(&transports, &transport) else {
        return value_error(format!(
            "manifest.interconnect.nccl_transport '{transport}' 미정의 — 알려진 {} (fail-loud · 오타가 조용히 socket 으로 가면 RDMA 를 쟀다고 믿게 된다)",
            py_repr_list(&sorted_names(&transports))
        ));
    };
    env.extend(t.clone());
    Ok(env)
}

pub(crate) fn type_name(v: &V) -> &'static str {
    match v {
        V::None => "NoneType",
        V::Bool(_) => "bool",
        V::Int(_) => "int",
        V::Float(_) => "float",
        V::Str(_) => "str",
        V::List(_) => "list",
        V::Dict(_) => "dict",
    }
}

fn body_lines(env: &Env) -> String {
    env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("\n")
}

fn assemble(header: Vec<String>, env: &Env) -> String {
    let mut lines = header;
    let body = body_lines(env);
    let mut out = lines.drain(..).collect::<Vec<_>>().join("\n");
    if !body.is_empty() {
        out.push('\n');
        out.push_str(&body);
    }
    out.push('\n');
    out
}

pub fn render_nccl_envfile(manifest: &V) -> Result<String, Raised> {
    let env = build_nccl_env(manifest)?;
    let preset_key = manifest.get("interconnect").and_then(|ic| ic.get("platform_preset")).map_or("None".to_string(), V::py_str);
    let header = vec![
        "# .env.interconnect — manifest-driven NCCL/RDMA env (render_dockerfile.py 생성·비추적).".to_string(),
        format!("# 원천: manifest.interconnect(①환경값) + NCCL_PRESETS[{preset_key}](②프리셋) + NCCL_INVARIANTS(③불변)."),
        "# 회귀 oracle: fixtures/nccl_env_dgx-spark-gb10.golden (집합 동치). 손수정 금지 — manifest/preset 을 고칠 것.".to_string(),
        String::new(),
    ];
    Ok(assemble(header, &env))
}

pub fn build_cluster_env(manifest: &V) -> Result<Env, Raised> {
    let nodes = match manifest.get("nodes") {
        Some(V::List(l)) if !l.is_empty() => l,
        _ => return value_error("manifest.nodes[] 부재 — .env.cluster 렌더 불가(fail-loud, 무증거 진행 금지)".into()),
    };
    let mut by_role: BTreeMap<String, &V> = BTreeMap::new();
    for n in nodes {
        let r = n.get("role").map_or(String::new(), V::py_str);
        let r = py_strip(&r).to_string();
        if !r.is_empty() {
            by_role.insert(r, n);
        }
    }
    let (main, sub) = (by_role.get("main").copied(), by_role.get("sub").copied());
    let (Some(main), Some(sub)) = (main.filter(|n| n.truthy()), sub.filter(|n| n.truthy())) else {
        let roles: Vec<&str> = by_role.keys().map(String::as_str).collect();
        return value_error(format!("manifest.nodes[] 에 role=main·sub 둘 다 필요(멀티 분산) — got roles={}", py_repr_list(&roles)));
    };
    let master_ip = py_strip(&get_str(main, "host", "")).to_string();
    let slave_ip = py_strip(&get_str(sub, "host", "")).to_string();
    let main_user = main.get("ssh_user").cloned().unwrap_or(V::Str(String::new()));
    let ssh_user = if main_user.truthy() {
        main_user.py_str()
    } else {
        get_str(sub, "ssh_user", "")
    };
    let ssh_user = py_strip(&ssh_user).to_string();
    if master_ip.is_empty() || slave_ip.is_empty() || ssh_user.is_empty() {
        return value_error(format!(
            "nodes[] host/ssh_user 결손 — master={} slave={} ssh_user={}",
            py_repr_str(&master_ip),
            py_repr_str(&slave_ip),
            py_repr_str(&ssh_user)
        ));
    }
    let mut env = Env::new();
    env.insert("MASTER_HOST_IP".into(), master_ip);
    env.insert("SLAVE_HOST_IP".into(), slave_ip);
    env.insert("SSH_USER".into(), ssh_user);
    let presets = cluster_presets();
    let platform = manifest.get("interconnect").filter(|v| v.truthy()).and_then(|ic| ic.get("platform_preset"));
    let preset = match platform {
        Some(V::None) | None => None,
        Some(v) => lookup(&presets, &v.py_str()),
    };
    let Some(preset) = preset else {
        let shown = platform.map_or("None".to_string(), V::py_str);
        return key_error(format!(
            "platform_preset '{shown}' 미정의 — 알려진 {} (fail-loud·오타 방지; 미검증 플랫폼은 'generic'(튜닝 0)으로 일단 기동→스모크 중재, 검증 후 전용 프리셋으로 승격)",
            py_repr_list(&sorted_names(&presets))
        ));
    };
    env.extend(preset.clone());
    env.extend(cluster_invariants());
    Ok(env)
}

pub fn render_cluster_envfile(manifest: &V) -> Result<String, Raised> {
    let env = build_cluster_env(manifest)?;
    let preset_key = manifest.get("interconnect").filter(|v| v.truthy()).and_then(|ic| ic.get("platform_preset")).map_or("None".to_string(), V::py_str);
    let header = vec![
        "# .env.cluster — manifest-driven Ray 클러스터-배포 env (render_dockerfile.py --cluster-envfile 생성·비추적).".to_string(),
        format!("# 원천: manifest.nodes[](①환경값) + CLUSTER_PRESETS[{preset_key}](②프리셋) + CLUSTER_INVARIANTS(③불변)."),
        "# Band2(토폴로지-keyed, 서브 전파). 손수정 금지 — manifest.nodes/preset 을 고칠 것.".to_string(),
        String::new(),
    ];
    Ok(assemble(header, &env))
}

// ── env_tier ───────────────────────────────────────────────────────────────────────────────────────

const ENV_TIER_PROBES: [(&str, &str); 7] = [
    ("__probe_hca__", "interconnect.hca_devices"),
    ("__probe_gid__", "interconnect.gid_index"),
    ("__probe_iface__", "interconnect.socket_iface"),
    ("__probe_main_host__", "nodes[main].host"),
    ("__probe_sub_host__", "nodes[sub].host"),
    ("__probe_main_ssh__", "nodes[main].ssh_user"),
    ("__probe_sub_ssh__", "nodes[sub].ssh_user"),
];

fn s(text: &str) -> V {
    V::Str(text.to_string())
}

fn dict(items: Vec<(&str, V)>) -> V {
    V::Dict(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn probe_env() -> Result<Env, Raised> {
    let nccl = nccl_presets();
    let cluster = cluster_presets();
    let mut common: Vec<&str> = nccl.iter().map(|(k, _)| *k).filter(|k| lookup(&cluster, k).is_some()).collect();
    common.sort_unstable();
    let Some(first) = common.first() else {
        return key_error("NCCL_PRESETS 와 CLUSTER_PRESETS 에 공통 프리셋이 없다 — env_tier 탐침 불가(fail-loud)".into());
    };
    let probe = dict(vec![
        (
            "interconnect",
            dict(vec![
                ("hca_devices", V::List(vec![s("__probe_hca__")])),
                ("gid_index", s("__probe_gid__")),
                ("socket_iface", s("__probe_iface__")),
                ("platform_preset", s(first)),
            ]),
        ),
        (
            "nodes",
            V::List(vec![
                dict(vec![("role", s("main")), ("host", s("__probe_main_host__")), ("ssh_user", s("__probe_main_ssh__"))]),
                dict(vec![("role", s("sub")), ("host", s("__probe_sub_host__")), ("ssh_user", s("__probe_sub_ssh__"))]),
            ]),
        ),
    ]);
    let mut env = build_nccl_env(&probe)?;
    env.extend(build_cluster_env(&probe)?);
    Ok(env)
}

/// `env_tier(key)` → `(tier, detail)`; an unknown key is `("unknown", None)`.
pub fn env_tier(key: &str) -> Result<(&'static str, Option<String>), Raised> {
    if nccl_invariants().contains_key(key) || cluster_invariants().contains_key(key) {
        return Ok(("invariant", None));
    }
    if nccl_transports().iter().any(|(_, keys)| keys.contains_key(key)) {
        return Ok(("env", Some("interconnect.nccl_transport".into())));
    }
    let mut names = BTreeSet::new();
    for table in [nccl_presets(), cluster_presets()] {
        for (name, keys) in &table {
            if keys.contains_key(key) {
                names.insert(*name);
            }
        }
    }
    if !names.is_empty() {
        return Ok(("preset", Some(names.into_iter().collect::<Vec<_>>().join(","))));
    }
    if let Some(value) = probe_env()?.get(key) {
        for (sentinel, field) in ENV_TIER_PROBES {
            if value.contains(sentinel) {
                return Ok(("env", Some(field.to_string())));
            }
        }
    }
    Ok(("unknown", None))
}

pub(crate) fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ── materializers ──────────────────────────────────────────────────────────────────────────────────

pub(crate) fn os_error(prefix: &str, error: &std::io::Error, path: &Path) -> Raised {
    let (class, errno) = match error.kind() {
        std::io::ErrorKind::NotFound => ("FileNotFoundError", 2),
        std::io::ErrorKind::PermissionDenied => ("PermissionError", 13),
        std::io::ErrorKind::AlreadyExists => ("FileExistsError", 17),
        _ => ("OSError", error.raw_os_error().unwrap_or(5)),
    };
    let _ = prefix;
    Raised { class, message: format!("[Errno {errno}] {}: {}", error.to_string().split(" (os error").next().unwrap_or_default(), py_repr_str(&path.to_string_lossy())) }
}

pub(crate) fn materialize_configs(skill_root: &Path, repo: &Path, topology: &str) -> Result<Vec<PathBuf>, Raised> {
    let src_dir = skill_root.join("assets").join("configs");
    let dst_dir = repo.join("output").join(topology).join("configs");
    std::fs::create_dir_all(&dst_dir).map_err(|e| os_error("makedirs", &e, &dst_dir))?;
    let mut copied = Vec::new();
    for name in RUNNER_SCRIPTS {
        let src = src_dir.join(name);
        if !src.is_file() {
            return Err(Raised { class: "FileNotFoundError", message: format!("shared runner source missing: {}", src.display()) });
        }
        let dst = dst_dir.join(name);
        std::fs::copy(&src, &dst).map_err(|e| os_error("copy", &e, &dst))?;
        std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755)).map_err(|e| os_error("chmod", &e, &dst))?;
        copied.push(dst);
    }
    Ok(copied)
}

fn join_path(base: &str, leaf: &str) -> String {
    if base.is_empty() || base.ends_with('/') {
        format!("{base}{leaf}")
    } else {
        format!("{base}/{leaf}")
    }
}

fn materialize_env(repo: &str, topology: &str, manifest: &V) -> Result<PathBuf, Raised> {
    let nas = py_strip(&get_str(manifest, "nas_model_path", "")).to_string();
    if nas.is_empty() {
        return value_error(format!(
            "manifest.nas_model_path 부재 — output/{topology}/.env materialize 불가. serve 가 compose 기본값 /mnt/models 를 마운트해 모델을 못 찾는다. manifest 를 채울 것."
        ));
    }
    let or_str = |key: &str| -> String {
        match manifest.get(key) {
            Some(v) if v.truthy() => py_strip(&v.py_str()).to_string(),
            _ => String::new(),
        }
    };
    let tiktoken = {
        let t = or_str("tiktoken_host_path");
        if t.is_empty() { join_path(repo, "tiktoken_cache") } else { t }
    };
    let declared = or_str("quant_model_path");
    let (quant, quant_source) = if declared.is_empty() {
        (nas.clone(), "FALLBACK:nas_model_path")
    } else {
        (declared.clone(), "manifest.quant_model_path")
    };
    if declared.is_empty() {
        gate_cli::write_err(&format!(
            "[render] WARN: manifest.quant_model_path 부재 — QUANT_MODEL_PATH 를 nas_model_path({nas}) 로 폴백한다.\n[render]   양자화 모델(/app/quant_models/...)을 쓰면 이 노드에서 경로가 해소되지 않는다. manifest 를 채울 것.\n"
        ));
    }
    let dst_dir = Path::new(repo).join("output").join(topology);
    std::fs::create_dir_all(&dst_dir).map_err(|e| os_error("makedirs", &e, &dst_dir))?;
    let dst = dst_dir.join(".env");
    let ple_mmap = or_str("ple_mmap_host_path");
    let mut body = format!(
        "# 프로젝트-레벨 env (compose 변수치환) — render_dockerfile.py --materialize-env 가 manifest 에서 생성.\n\
# docker compose 가 docker-compose.yaml 의 ${{NAS_MODEL_PATH}}·${{QUANT_MODEL_PATH}}·${{TIKTOKEN_HOST_PATH}} 치환에 사용.\n\
# gitignored(output/* — PII). 손수정 금지 — manifest(nas_model_path·quant_model_path·tiktoken_host_path·ple_mmap_host_path)를 고칠 것.\n\
NAS_MODEL_PATH={nas}\n\
# QUANT_MODEL_PATH 출처(결정론 규율 — 값 옆에 출처): {quant_source}\n\
QUANT_MODEL_PATH={quant}\n\
TIKTOKEN_HOST_PATH={tiktoken}\n"
    );
    if !ple_mmap.is_empty() {
        body.push_str(&format!(
            "# PLE_MMAP_HOST_PATH 출처(결정론 규율 — 값 옆에 출처): manifest.ple_mmap_host_path\nPLE_MMAP_HOST_PATH={ple_mmap}\n"
        ));
    }
    std::fs::write(&dst, body).map_err(|e| os_error("open", &e, &dst))?;
    Ok(dst)
}

// ── command line ───────────────────────────────────────────────────────────────────────────────────

fn opt(flags: &'static [&'static str], dest: &'static str, kind: Kind) -> Opt {
    Opt { flags, dest, kind, help: None, required: false, group: None }
}

pub fn spec() -> Spec {
    Spec {
        prog: PROGRAM,
        description: "render_dockerfile.py — G2 결정론 렌더러",
        options: vec![
            opt(&["-h", "--help"], "help", Kind::Help),
            opt(&["--self-test"], "self_test", Kind::Flag),
            opt(&["--nccl-envfile"], "nccl_envfile", Kind::Flag),
            opt(&["--cluster-envfile"], "cluster_envfile", Kind::Flag),
            opt(&["--materialize-configs"], "materialize_configs", Kind::Flag),
            opt(&["--materialize-env"], "materialize_env", Kind::Flag),
            opt(&["--topology"], "topology", Kind::Choice(&["single", "multi"])),
            opt(&["--repo"], "repo", Kind::Text),
            opt(&["--template"], "template", Kind::Text),
            opt(&["--canonical-kind"], "canonical_kind", Kind::Choice(&["dockerfile", "source-build", "compose"])),
            opt(&["--manifest"], "manifest", Kind::Text),
            opt(&["--resolved"], "resolved", Kind::Text),
            opt(&["-o", "--out"], "out", Kind::Text),
            opt(&["--allow-unvalidated"], "allow_unvalidated", Kind::Flag),
            // new in the port (the original exposes `env_tier` only as a function): the tier of one env key as a JSON line
            opt(&["--env-tier"], "env_tier", Kind::Text),
        ],
        group_required: false,
        positional: None,
    }
}

pub(crate) fn report(raised: Raised) -> i32 {
    if raised.class == "SystemExit" {
        // `raise SystemExit("text")`: the text alone on stderr, status 1
        gate_cli::write_err(&format!("{}\n", raised.message));
        return 1;
    }
    gate_cli::write_err(&format!("{}: {}\n", raised.class, raised.message));
    1
}

fn write_envfile(values: &Values, label: &str, text: &str) -> i32 {
    match gate_cli::text(values, "out") {
        Some(out) => {
            if let Err(e) = std::fs::write(&out, text) {
                return report(os_error("open", &e, Path::new(&out)));
            }
            let keys: BTreeSet<&str> = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| l.split_once('=').map_or(l, |(k, _)| k).trim())
                .collect();
            gate_cli::write_err(&format!("[render] {label} → {out} ({} keys)\n", keys.len()));
        }
        None => gate_cli::write_out(text),
    }
    0
}

pub fn run(script: Option<&PyStr>, args: &[PyStr]) -> i32 {
    let values: Values = match gate_cli::parse_args(&spec(), args) {
        Ok(v) => v,
        Err(status) => return status,
    };
    if gate_cli::flag(&values, "allow_unvalidated") {
        gate_cli::write_err("[render] --allow-unvalidated: source-build 가드 WARN 강등(HITL 시도-빌드 — testlog 기록 의무)\n");
    }
    let manifest_path = gate_cli::text(&values, "manifest").unwrap_or_else(|| "manifest.yaml".into());
    let topology = gate_cli::text(&values, "topology");
    let scripts = gate_cli::scripts_dir(script, "render_dockerfile");
    let result: Result<i32, Raised> = (|| {
        if let Some(key) = gate_cli::text(&values, "env_tier") {
            let (tier, detail) = env_tier(&key)?;
            let detail = detail.map_or("null".to_string(), |d| json_string(&d));
            gate_cli::write_out(&format!("{{\"tier\": {}, \"detail\": {detail}}}\n", json_string(tier)));
            return Ok(0);
        }
        if gate_cli::flag(&values, "self_test") {
            return Ok(crate::render_template::selftest::self_test(&scripts));
        }
        if gate_cli::flag(&values, "nccl_envfile") {
            let text = render_nccl_envfile(&load_manifest(&manifest_path)?)?;
            return Ok(write_envfile(&values, "NCCL envfile", &text));
        }
        if gate_cli::flag(&values, "cluster_envfile") {
            let text = render_cluster_envfile(&load_manifest(&manifest_path)?)?;
            return Ok(write_envfile(&values, "cluster envfile", &text));
        }
        let repo = || -> String {
            gate_cli::text(&values, "repo").unwrap_or_else(|| {
                let root = scripts.ancestors().nth(4).map(Path::to_path_buf).unwrap_or_else(|| scripts.clone());
                root.to_string_lossy().into_owned()
            })
        };
        if gate_cli::flag(&values, "materialize_configs") {
            let Some(topology) = topology.as_deref() else {
                gate_cli::write_err("[render] FAIL: --materialize-configs 에는 --topology {single|multi} 필요\n");
                return Ok(2);
            };
            let skill_root = scripts.parent().map(Path::to_path_buf).unwrap_or_else(|| scripts.clone());
            for c in materialize_configs(&skill_root, Path::new(&repo()), topology)? {
                gate_cli::write_err(&format!("[render] materialize → {}\n", c.display()));
            }
            return Ok(0);
        }
        if gate_cli::flag(&values, "materialize_env") {
            let Some(topology) = topology.as_deref() else {
                gate_cli::write_err("[render] FAIL: --materialize-env 에는 --topology {single|multi} 필요\n");
                return Ok(2);
            };
            let manifest = load_manifest(&manifest_path)?;
            let env_path = materialize_env(&repo(), topology, &manifest)?;
            gate_cli::write_err(&format!("[render] materialize env → {}\n", env_path.display()));
            return Ok(0);
        }
        if let Some(status) = crate::render_template::run_render(&values, &scripts, &manifest_path, gate_cli::flag(&values, "allow_unvalidated"))? {
            return Ok(status);
        }
        Ok(crate::render_template::selftest::self_test(&scripts))
    })();
    match result {
        Ok(status) => status,
        Err(raised) => report(raised),
    }
}
