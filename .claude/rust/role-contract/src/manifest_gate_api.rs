//! The in-process importers' view of `manifest_contract.py`: the same decisions as the command line, over arguments of ANY
//! Python value, answered as DATA that a thin Python adapter assembles mechanically.
//!
//! * `evaluate_contract` answers a result PLAN: for every key of the owner's `res` dictionary either a reference to an ORIGINAL
//!   input object (`man.get(name)`: identity, aliases, cycles and YAML-only types stay Python's own), or a typed value this side
//!   computed. Whether a falsy default replaces an input, which role names which `effective_model_source` entry and which source it
//!   takes are decided HERE; the adapter only fetches fields and assigns dictionary entries.
//! * `manifest_tp`, `effective_model_source`, `resolve_topology` and `load_manifest` answer a value, a reference or a failure.
//!
//! Nothing here knows a fixture, a path or an expected answer.

use super::*;

/// Which role object names an `effective_model_source` entry.
#[derive(Clone, Debug, PartialEq)]
pub enum RoleSource {
    /// `node.get("role")`
    NodeRole,
    /// the text `"node%d" % i`
    Fallback(String),
}

/// Which object an `effective_model_source` entry maps to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueSource {
    /// `node["model_source"]`
    NodeModelSource,
    /// the top-level `model_source` of the manifest
    TopModelSource,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveEntry {
    pub node: usize,
    pub role: RoleSource,
    pub value: ValueSource,
}

/// One entry of the result plan, in the owner's key order.
#[derive(Clone, Debug)]
pub enum PlanEntry {
    /// `man.get(name)`: the original object.
    Input { key: String, name: String },
    /// A value this side computed (a fresh object in Python).
    Value { key: String, value: PyValue },
    /// `{role-or-fallback: source, ...}` over `res["nodes"]`, entries in order.
    EffectiveMap {
        key: String,
        entries: Vec<EffectiveEntry>,
    },
}

/// `evaluate_contract(man, topology)` as a plan.
pub fn evaluate_contract_plan(
    man: &PyValue,
    topology: &PyValue,
) -> Result<Vec<PlanEntry>, GateFailure> {
    let evaluation = evaluate_contract_value(man, topology)?;
    let manifest = as_dict(man).expect("an evaluation succeeds only for a mapping");
    let mut plan = Vec::new();
    for (key, value) in evaluation.res.iter() {
        let name = key.to_utf8().unwrap_or_default();
        match name.as_str() {
            "topology" | "gpus_per_node" | "model_source" | "nas_model_path" => {
                plan.push(PlanEntry::Input {
                    key: name.clone(),
                    name,
                })
            }
            // `man.get(name) or default`: the input when it is truthy, the default otherwise
            "nodes" | "custom_model_paths" | "quant_model_path" | "tiktoken_host_path" => {
                if truthy(manifest.get(&name)) {
                    plan.push(PlanEntry::Input {
                        key: name.clone(),
                        name,
                    })
                } else {
                    plan.push(PlanEntry::Value {
                        key: name,
                        value: value.clone(),
                    })
                }
            }
            "flag" | "reason" | "exit_code" | "manifest_tp" | "node_warnings" => {
                plan.push(PlanEntry::Value {
                    key: name,
                    value: value.clone(),
                })
            }
            "effective_model_source" => {
                let nodes = evaluation
                    .res
                    .get("nodes")
                    .cloned()
                    .unwrap_or(PyValue::None);
                let entries = dict_members(&nodes)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(index, node)| EffectiveEntry {
                        node: index,
                        role: if node.get("role").is_some_and(PyValue::is_truthy) {
                            RoleSource::NodeRole
                        } else {
                            RoleSource::Fallback(format!("node{index}"))
                        },
                        value: if node.get("model_source").is_some_and(PyValue::is_truthy) {
                            ValueSource::NodeModelSource
                        } else {
                            ValueSource::TopModelSource
                        },
                    })
                    .collect();
                plan.push(PlanEntry::EffectiveMap { key: name, entries });
            }
            other => {
                return Err(GateFailure::Unsupported {
                    boundary: "result_key",
                    detail: format!("the result plan has no source for the key {other:?}"),
                })
            }
        }
    }
    Ok(plan)
}

/// `manifest_tp(man, topology)`: a number or `None`, or the exception the owner raises.
pub fn manifest_tp_answer(man: &PyValue, topology: &PyValue) -> Result<PyValue, GateFailure> {
    let Some(manifest) = as_dict(man) else {
        return Err(at(
            no_attribute(man.peel(), "get", vec![MAN_GET]),
            vec![Step::Arg(0)],
        ));
    };
    match manifest_tp(manifest, topology)? {
        Tp::Value(value) => Ok(value),
        Tp::ValueError(text) => Err(raised("ValueError", text, vec![CALL_MANIFEST_TP])),
    }
}

/// `effective_model_source(node, top_level_ms)`: which of the two objects comes back.
pub fn effective_model_source_which(node: &PyValue, _top_level: &PyValue) -> ValueSource {
    match as_dict(node) {
        Some(map) if map.get("model_source").is_some_and(PyValue::is_truthy) => {
            ValueSource::NodeModelSource
        }
        _ => ValueSource::TopModelSource,
    }
}

/// What `resolve_topology(repo, explicit)` returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TopologyAnswer {
    /// the `explicit` object itself (it is truthy)
    Explicit,
    /// `"single"`, `"multi"` or `None`
    Branch(Option<String>),
}

pub fn resolve_topology_answer(
    probe: &mut dyn BranchProbe,
    repo: &[u8],
    explicit: &PyValue,
) -> TopologyAnswer {
    if explicit.is_truthy() {
        TopologyAnswer::Explicit
    } else {
        TopologyAnswer::Branch(resolve_topology(probe, repo, None))
    }
}

/// `_load_manifest(path)`: `yaml.safe_load(open(path, "r", encoding="utf-8")) or {}`.
pub fn load_manifest_answer(path: &PyValue) -> Result<PyValue, crate::load::LoadFailure> {
    let value = crate::load::load_yaml_objects(path)?;
    Ok(if value.is_truthy() {
        value
    } else {
        PyValue::Dict(PyDict::new())
    })
}
