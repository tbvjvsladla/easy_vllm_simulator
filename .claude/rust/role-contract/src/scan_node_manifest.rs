//! P1 manifest assembly of `scan_node.py`: `build_sub_manifest` (the sub node's manifest from facts the main node measured) and
//! `emit_manifest_block` (the YAML block of a verified scan). Both read the local clock hour only through [`Clock`], and only when the
//! original reads it (`scanned_at or datetime.now()...`; `emit_manifest_block` always).

use crate::scan_node::{
    attribute_error, derive_platform_preset, dict_get, dict_get_or, dict_of, equals_text, in_texts,
    is_none, or, raise, shown, subscript, text, yaml_str, Failure, Res,
};
use agent_runtime_support::pydyn::{iterate, Text};
use agent_runtime_support::pystr::{is_decimal_digit, PyStr};
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The local wall-clock hour as `%Y%m%d%H`: the second boundary of P1 (the first is the file reads). The pure functions never see it.
pub trait Clock {
    fn local_hour(&self) -> PyStr;
}

/// `re.search(r"(\d+)\.(\d+)", text)` as the two digit runs joined (`f"{major}{minor}"`), or `None`.
fn cuda_digits(shown_text: &PyStr) -> Option<PyStr> {
    let points = shown_text.code_points();
    let dot = u32::from(b'.');
    let mut start = 0;
    while start < points.len() {
        if !is_decimal_digit(points[start]) {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < points.len() && is_decimal_digit(points[end]) {
            end += 1;
        }
        if points.get(end) == Some(&dot) {
            let mut tail = end + 1;
            while tail < points.len() && is_decimal_digit(points[tail]) {
                tail += 1;
            }
            if tail > end + 1 {
                let mut joined = points[start..end].to_vec();
                joined.extend_from_slice(&points[end + 1..tail]);
                return Some(PyStr::from_code_points(joined));
            }
        }
        // A start inside the same digit run cannot do better than this one; resume after it.
        start = end;
    }
    None
}

/// `dict(value)` of a value the original copies: a dict is copied, a sequence of pairs is paired up, anything else raises as CPython does.
fn dict_copy(value: &PyValue) -> Res<PyValue> {
    match value.peel() {
        PyValue::Dict(map) => Ok(PyValue::Dict(map.clone())),
        PyValue::None | PyValue::Bool(_) | PyValue::Int(_) | PyValue::Float(_) => Err(raise(
            "TypeError",
            &format!("'{}' object is not iterable", value.type_name()),
        )),
        PyValue::List(items) | PyValue::Tuple(items) => {
            let mut out = PyDict::new();
            for (index, item) in items.iter().enumerate() {
                let pair = match item.peel() {
                    PyValue::List(inner) | PyValue::Tuple(inner) => inner.clone(),
                    PyValue::Str(own) if own.len() == 2 => own
                        .code_points()
                        .iter()
                        .map(|&point| PyValue::Str(PyStr::from_code_points(vec![point])))
                        .collect(),
                    PyValue::Dict(_) | PyValue::Str(_) | PyValue::Bytes(_) => iterate(item)?,
                    _ => {
                        return Err(raise(
                            "TypeError",
                            &format!(
                                "cannot convert dictionary update sequence element #{index} to a sequence"
                            ),
                        ))
                    }
                };
                let [key, member] = pair.as_slice() else {
                    return Err(raise(
                        "ValueError",
                        &format!(
                            "dictionary update sequence element #{index} has length {}; 2 is required",
                            pair.len()
                        ),
                    ));
                };
                let PyValue::Str(key) = key.peel() else {
                    return Err(Failure::Unsupported(
                        "dict() of a pair with a non-string key is not modelled".into(),
                    ));
                };
                out.insert(key.clone(), member.clone());
            }
            Ok(PyValue::Dict(out))
        }
        PyValue::Str(own) => {
            if own.is_empty() {
                Ok(PyValue::Dict(PyDict::new()))
            } else {
                Err(raise(
                    "ValueError",
                    "dictionary update sequence element #0 has length 1; 2 is required",
                ))
            }
        }
        other => Err(Failure::Unsupported(format!(
            "dict() of a {} is not modelled",
            other.type_name()
        ))),
    }
}

/// The items `for n in value` yields.
fn items_of(value: &PyValue) -> Res<Vec<PyValue>> {
    Ok(iterate(value)?)
}

/// `.get(key)` where `key` is any value: strings look up, an unhashable key is a `TypeError`, any other key finds nothing in a string-keyed dict.
fn dict_get_any(map: &PyValue, key: &PyValue) -> Res<PyValue> {
    match key.peel() {
        PyValue::Str(own) => match own.to_utf8() {
            Some(name) => dict_get(map, &name),
            None => match map.peel() {
                PyValue::Dict(dict) => Ok(dict
                    .iter()
                    .find(|(candidate, _)| candidate == own)
                    .map(|(_, value)| value.clone())
                    .unwrap_or(PyValue::None)),
                _ => Err(attribute_error(map, "get")),
            },
        },
        PyValue::List(_) | PyValue::Dict(_) => {
            dict_get(map, "")?;
            Err(raise(
                "TypeError",
                &format!("unhashable type: '{}'", key.type_name()),
            ))
        }
        _ => {
            dict_get(map, "")?;
            Ok(PyValue::None)
        }
    }
}

/// `build_sub_manifest(result, peer_hw, main_manifest, scanned_at, egress_peer)`.
pub fn build_sub_manifest(
    result: &PyValue,
    peer_hw: &PyValue,
    main_manifest: &PyValue,
    scanned_at: &PyValue,
    egress_peer: &PyValue,
    clock: &dyn Clock,
) -> Res<PyValue> {
    let cuda_raw = dict_get(peer_hw, "cuda")?;
    let searched = if cuda_raw.is_truthy() {
        shown(&cuda_raw)
    } else {
        PyStr::default()
    };
    let cuda = cuda_digits(&searched).map_or(PyValue::None, PyValue::Str);
    let homo = or(dict_get(result, "homogeneity")?, || {
        Ok(PyValue::Dict(PyDict::new()))
    })?;
    let topology = or(dict_get(result, "declared_topology")?, || {
        or(dict_get(result, "topology")?, || {
            dict_get(main_manifest, "topology")
        })
    })?;
    let mut man = PyDict::new();
    man.set("self_role", text("sub"));
    man.set("topology", topology);
    man.set("cpu_arch", dict_get(peer_hw, "cpu_arch")?);
    man.set("cuda_version", cuda);
    man.set("gpus_per_node", dict_get(peer_hw, "gpus_per_node")?);
    man.set("gpu_model", dict_get(peer_hw, "gpu_model")?);
    man.set("driver_version", dict_get(peer_hw, "driver")?);
    man.set("gpu_memory_model", dict_get(peer_hw, "gpu_memory_model")?);
    man.set("gpu_vram_mib", dict_get(peer_hw, "gpu_vram_mib")?);
    man.set(
        "compute_capability",
        dict_get(peer_hw, "compute_capability")?,
    );
    let scanned = or(scanned_at.clone(), || Ok(PyValue::Str(clock.local_hour())))?;
    let verified_rule = or(dict_get(&homo, "verified_rule")?, || {
        Ok(text("single:peer-observed(gpus>=1)"))
    })?;
    man.set(
        "terraforming",
        dict_of(vec![
            ("complete", PyValue::Bool(true)),
            ("branch_verified", PyValue::Bool(true)),
            ("scanned_at", scanned),
            ("issued_by", text("main")),
            ("verified_rule", verified_rule),
        ]),
    );
    man.set("model_source", dict_get(result, "model_source")?);
    man.set(
        "nas_model_path",
        or(dict_get(result, "nas_model_path")?, || {
            dict_get(main_manifest, "nas_model_path")
        })?,
    );
    man.set(
        "custom_model_paths",
        or(dict_get(main_manifest, "custom_model_paths")?, || {
            Ok(PyValue::Dict(PyDict::new()))
        })?,
    );
    for (key, name) in [
        ("hf_token_env_file", "hf_token_env_file"),
        ("quant_model_path", "quant_model_path"),
        ("tiktoken_host_path", "tiktoken_host_path"),
    ] {
        man.set(key, or(dict_get(main_manifest, name)?, || Ok(text("")))?);
    }
    if egress_peer.is_truthy() {
        man.set(
            "network",
            dict_of(vec![
                ("egress", egress_peer.clone()),
                ("egress_source", text("measured:peer-probe")),
            ]),
        );
    }
    if dict_get(main_manifest, "origin_url")?.is_truthy() {
        man.set("origin_url", subscript(main_manifest, "origin_url")?);
    }
    if dict_get(main_manifest, "interconnect")?.is_truthy() {
        man.set(
            "interconnect",
            dict_copy(&subscript(main_manifest, "interconnect")?)?,
        );
    }
    let nodes_source = or(dict_get(result, "nodes")?, || Ok(PyValue::List(vec![])))?;
    let mut nodes = Vec::new();
    for node in items_of(&nodes_source)? {
        nodes.push(dict_copy(&node)?);
    }
    man.set("nodes", PyValue::List(nodes.clone()));
    let lib = or(dict_get(main_manifest, "library")?, || {
        Ok(PyValue::Dict(PyDict::new()))
    })?;
    let main_nodes = or(dict_get(main_manifest, "nodes")?, || {
        Ok(PyValue::List(vec![]))
    })?;
    let mut main_node = PyValue::Dict(PyDict::new());
    for node in items_of(&main_nodes)? {
        if equals_text(&dict_get(&node, "role")?, "main") {
            main_node = node;
            break;
        }
    }
    let mut sub_node = PyValue::Dict(PyDict::new());
    for node in &nodes {
        if equals_text(&dict_get(node, "role")?, "sub") {
            sub_node = node.clone();
            break;
        }
    }
    let nid = or(dict_get(&sub_node, "node_id")?, || Ok(text("sub")))?;
    let access_points = or(dict_get(&lib, "access_points")?, || {
        Ok(PyValue::Dict(PyDict::new()))
    })?;
    let ap = dict_get_any(&access_points, &nid)?;
    if ap.is_truthy() && dict_get(&main_node, "host")?.is_truthy() {
        let host = subscript(&main_node, "host")?;
        let user = dict_get(&main_node, "ssh_user")?;
        let repo_root = or(dict_get(&lib, "repo_root")?, || {
            dict_get(&main_node, "work_dir")
        })?;
        let wiki_dir = or(dict_get(&lib, "wiki_dir")?, || Ok(text("__llm-wiki")))?;
        let identity_file = dict_get(&ap, "identity_file")?;
        let known_hosts = dict_get(&ap, "known_hosts")?;
        man.set(
            "library",
            dict_of(vec![
                ("transport", text("ssh")),
                ("host", host),
                ("user", user),
                ("repo_root", repo_root),
                ("wiki_dir", wiki_dir),
                ("identity_file", identity_file),
                ("known_hosts", known_hosts),
                ("node_id", nid),
            ]),
        );
    }
    Ok(PyValue::Dict(man))
}

fn cuda_field(value: &PyValue) -> PyStr {
    Text::new().lit("\"").val(value).lit("\"").done()
}

fn line(parts: Text) -> PyStr {
    parts.done()
}

/// `emit_manifest_block(result)`: the YAML block of a verified scan. The clock is read once, where the original reads it.
pub fn emit_manifest_block(result: &PyValue, clock: &dyn Clock) -> Res<PyValue> {
    let ic = subscript(result, "interconnect")?;
    let hca_items = subscript(&ic, "hca_devices")?;
    let joined = agent_runtime_support::pydyn::join(", ", &hca_items)?;
    let hcas = Text::new().lit("[").str(&joined).lit("]").done();
    let topo = if equals_text(&subscript(result, "topology_declared")?, "multi") {
        "multi"
    } else {
        "single"
    };
    let cuda_value = subscript(result, "cuda_version")?;
    let cuda = if !is_none(&subscript(result, "cuda_version")?) {
        cuda_field(&subscript(result, "cuda_version")?)
    } else {
        PyStr::new("null")
    };
    let _ = cuda_value;
    let gpus_value = subscript(result, "gpus_per_node")?;
    let gpus = if !is_none(&subscript(result, "gpus_per_node")?) {
        shown(&gpus_value)
    } else {
        PyStr::new("null")
    };
    let gpu_model = yaml_str(&dict_get(result, "gpu_model")?);
    let driver = yaml_str(&dict_get(result, "driver_version")?);
    let scanned_at = clock.local_hour();
    let mut lines: Vec<PyStr> = Vec::new();
    let mut push = |parts: Text| lines.push(line(parts));
    push(Text::new().lit("# 테라포밍 완수 Flag (attestation · plan_26063018) — scan §1.5 3자일치 통과 시 기입(보수적)."));
    push(Text::new().lit("terraforming:"));
    push(Text::new().lit("  complete: true"));
    push(
        Text::new()
            .lit("  branch_verified: true   # git 브랜치 ↔ topology ↔ scan 3자일치 단언 통과"),
    );
    push(
        Text::new()
            .lit("  scanned_at: \"")
            .str(&scanned_at)
            .lit("\""),
    );
    push(Text::new().lit("self_role: main"));
    push(Text::new().lit("topology: ").lit(topo));
    let cpu_arch = subscript(result, "cpu_arch")?;
    push(Text::new().lit("cpu_arch: \"").val(&cpu_arch).lit("\""));
    push(Text::new().lit("cuda_version: ").str(&cuda));
    push(Text::new().lit("gpus_per_node: ").str(&gpus));
    push(Text::new().lit("gpu_model: ").str(&gpu_model));
    push(Text::new().lit("driver_version: ").str(&driver));
    let memory_model = yaml_str(&dict_get(result, "gpu_memory_model")?);
    push(
        Text::new()
            .lit("gpu_memory_model: ")
            .str(&memory_model)
            .lit("   # unified(GB10 류 · memory.total [N/A]) | discrete"),
    );
    let vram_probe = dict_get(result, "gpu_vram_mib")?;
    let vram = if !is_none(&vram_probe) {
        shown(&subscript(result, "gpu_vram_mib")?)
    } else {
        PyStr::new("null")
    };
    push(Text::new().lit("gpu_vram_mib: ").str(&vram));
    let compute = yaml_str(&dict_get(result, "compute_capability")?);
    push(
        Text::new()
            .lit("compute_capability: ")
            .str(&compute)
            .lit("   # nvidia-smi compute_cap → resolve_build_track"),
    );
    let ms = dict_get(result, "model_source")?;
    if in_texts(&ms, &["managed", "ephemeral", "custom"]) {
        push(Text::new().lit("model_source: ").val(&ms));
    } else {
        push(Text::new().lit("model_source: __REQUIRED__   # managed|ephemeral|custom — 인터뷰 확정값. 미기입 시 manifest_contract exit 5(3 런타임 스킬 info-only)"));
    }
    let nas = dict_get(result, "nas_model_path")?;
    if nas.is_truthy() {
        push(Text::new().lit("nas_model_path: ").str(&yaml_str(&nas)));
    } else {
        push(Text::new().lit("nas_model_path: __REQUIRED__   # render_sub_env NAS_MOUNT 필수 — 미기입 시 렌더 exit 2"));
    }
    let egress_all = or(dict_get(result, "egress")?, || {
        Ok(PyValue::Dict(PyDict::new()))
    })?;
    let eg = or(dict_get(&egress_all, "self")?, || {
        Ok(PyValue::Dict(PyDict::new()))
    })?;
    if dict_get(&eg, "egress")?.is_truthy() {
        push(Text::new().lit("network:"));
        push(
            Text::new()
                .lit("  egress: ")
                .val(&subscript(&eg, "egress")?)
                .lit("          # --check-egress 실측(hf_reachable/gateway_reachable)"),
        );
    }
    push(Text::new().lit("nodes:"));
    let nodes_value = or(dict_get_or(result, "nodes", PyValue::List(vec![]))?, || {
        Ok(PyValue::List(vec![]))
    })?;
    for node in items_of(&nodes_value)? {
        let role = subscript(&node, "role")?;
        push(Text::new().lit("  - role: ").val(&role));
        push(
            Text::new()
                .lit("    host: \"")
                .val(&subscript(&node, "host")?)
                .lit("\""),
        );
        if dict_get(&node, "node_id")?.is_truthy() {
            push(
                Text::new()
                    .lit("    node_id: ")
                    .str(&yaml_str(&dict_get(&node, "node_id")?)),
            );
        } else {
            push(
                Text::new().lit(
                    "    node_id: __REQUIRED__   # established propagation exact-scope identity",
                ),
            );
        }
        if equals_text(&role, "sub") {
            let shown_host = if dict_get(&node, "hostname")?.is_truthy() {
                yaml_str(&dict_get(&node, "hostname")?)
            } else {
                PyStr::new("__REQUIRED__")
            };
            push(
                Text::new()
                    .lit("    hostname: ")
                    .str(&shown_host)
                    .lit("   # CLAUDE.template SUB_HOSTNAME 계약"),
            );
        } else {
            push(
                Text::new()
                    .lit("    hostname: ")
                    .str(&yaml_str(&dict_get(&node, "hostname")?)),
            );
        }
        if dict_get(&node, "ssh_user")?.is_truthy() {
            push(
                Text::new()
                    .lit("    ssh_user: ")
                    .str(&yaml_str(&dict_get(&node, "ssh_user")?)),
            );
        } else {
            push(
                Text::new()
                    .lit("    ssh_user: __REQUIRED__   # sync_to_sub 주소 해소·A2A 위임 계정"),
            );
        }
        if dict_get(&node, "work_dir")?.is_truthy() {
            push(
                Text::new()
                    .lit("    work_dir: ")
                    .str(&yaml_str(&dict_get(&node, "work_dir")?)),
            );
        } else {
            push(Text::new().lit("    work_dir: __REQUIRED__   # 서브 프로젝트 절대경로(--sub-work-dir 로 주입 가능)"));
        }
        if equals_text(&role, "sub") {
            let homogeneity = dict_get_or(result, "homogeneity", PyValue::Dict(PyDict::new()))?;
            let hv = dict_get(&homogeneity, "verified")?;
            if hv.is_truthy() {
                let ph = subscript(&subscript(result, "homogeneity")?, "peer")?;
                push(Text::new().lit("    hw_verified: true        # --peer-ssh 동질성 단언 통과 → A2A 위임 키 발급 자격"));
                push(
                    Text::new()
                        .lit("    gpu_model: ")
                        .str(&yaml_str(&dict_get(&ph, "gpu_model")?)),
                );
                push(
                    Text::new()
                        .lit("    driver_version: ")
                        .str(&yaml_str(&dict_get(&ph, "driver")?)),
                );
                push(
                    Text::new()
                        .lit("    cuda_version_raw: ")
                        .str(&yaml_str(&dict_get(&ph, "cuda")?)),
                );
            } else {
                push(Text::new().lit("    hw_verified: false       # --peer-ssh 동질성 미검증 → 위임 키 미발급(info-only)"));
            }
        }
    }
    if !dict_get(result, "nodes")?.is_truthy() {
        push(Text::new().lit("  []   # 노드 미등록. single 도 서브 등록이 가능하다(§2.7.0) — 등록 시 위 형식으로 추가"));
    }
    let preset_pair = derive_platform_preset(&dict_get(result, "gpu_model")?)?;
    let (preset, preset_source) = match preset_pair.peel() {
        PyValue::Tuple(pair) => (pair[0].clone(), pair[1].clone()),
        _ => unreachable!("derive_platform_preset answers a pair"),
    };
    push(Text::new().lit("interconnect:"));
    push(Text::new().lit("  type: ").val(&subscript(&ic, "type")?));
    push(Text::new().lit("  hca_devices: ").str(&hcas));
    let gid = subscript(&ic, "gid_index")?;
    push(
        Text::new()
            .lit("  gid_index: ")
            .str(&if !is_none(&subscript(&ic, "gid_index")?) {
                shown(&gid)
            } else {
                PyStr::new("null")
            }),
    );
    let socket = subscript(&ic, "socket_iface")?;
    push(
        Text::new()
            .lit("  socket_iface: ")
            .str(&if !is_none(&subscript(&ic, "socket_iface")?) {
                shown(&socket)
            } else {
                PyStr::new("null")
            }),
    );
    let mtu = dict_get(&ic, "mtu")?;
    push(Text::new().lit("  mtu: ").str(&if !is_none(&mtu) {
        shown(&subscript(&ic, "mtu")?)
    } else {
        PyStr::new("null")
    }));
    let bandwidth_text = if !is_none(&subscript(&ic, "bandwidth_gbps")?) {
        shown(&subscript(&ic, "bandwidth_gbps")?)
    } else {
        PyStr::new("null")
    };
    push(Text::new().lit("  bandwidth_gbps: ").str(&bandwidth_text));
    let bandwidth_source = if !is_none(&subscript(&ic, "bandwidth_gbps")?) {
        "measured:ib_write_bw-injected"
    } else {
        "null"
    };
    push(
        Text::new()
            .lit("  bandwidth_source: ")
            .lit(bandwidth_source),
    );
    push(Text::new().lit("  platform_preset: ").val(&preset));
    push(
        Text::new()
            .lit("  platform_preset_source: ")
            .val(&preset_source)
            .lit("   # 결정론 파생(헌법 §결정론 규율 — 값 옆에 출처)"),
    );
    push(Text::new().lit("# ⚠ `__REQUIRED__` 가 남아 있으면 그 manifest 는 아직 불완전하다 — 하류(manifest_contract·render_sub_env)가"));
    push(Text::new().lit("#   그 자리에서 fail-closed 한다. 값을 채우거나 해당 스캔 플래그(--model-source·--peer-ssh·--sub-work-dir)를 주고 재스캔한다."));
    push(Text::new().lit("# ⚠ host_safety.installed(true|false): terraforming 세션 최종 Y/N 답변 후 기입(scan 무증거 기입 ✗ · Flag 와 독립 —"));
    push(Text::new().lit("#   안전체계 미설치여도 Flag valid. 헌법 §호스트 안전체계 따름정리 선택화 · plan_26071115)."));
    let mut out = PyStr::default();
    for (index, each) in lines.iter().enumerate() {
        if index > 0 {
            out.push_str("\n");
        }
        out.push_py(each);
    }
    out.push_str("\n");
    Ok(PyValue::Str(out))
}
