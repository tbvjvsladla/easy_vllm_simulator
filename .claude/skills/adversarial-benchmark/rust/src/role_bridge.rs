//! The bridge between `classify_cell` and the native node-role library (`role-contract`, over the common package Q).
//!
//! **Policy is not here.** `role-contract` is the single definition of the node contract; this module only maps its
//! values and failures into the shapes `classify_cell` reads. The two value families differ (Q's `PyValue` against this
//! crate's), so the conversion is explicit and *lazy*: a document that `load_yaml` returned stays one shared Q value
//! ([`Arc`]) and a [`Descriptor`](crate::classify_cell_owner::Descriptor) container is a [`Node`] - the shared root and
//! the path to the value - never a copy. Only what the classifier asks for is converted: a scalar when it is read, a
//! tuple (a node role is judged by its content) when it is materialised. An unused 617-digit key, an unused
//! self-referencing anchor and everything else the original's code never touched are never converted, so they cannot
//! change a plan.
//!
//! **Failures.** An exception the library raises keeps its Python class and text (`ValueError`, `ParserError`, ...); a
//! situation outside the library's named profile, a query on a value that is not what it was asked to be, and a
//! self-referencing value that is materialised on demand are the named [`BRIDGE_ERROR`], never a business error and
//! never a fallback to a Python owner.
//!
//! **Check-only seam.** [`RoleSource`] is the two operations the classifier needs (`load_yaml`, `evaluate_manifest`).
//! [`FixedRoleContract`] is the production source. A check may install another one (`Owners::with_role_source`) to hand
//! the classifier crafted typed values; no production path does.

use crate::classify_cell_owner::{Descriptor, Raised};
use crate::guidellm::Failure;
use crate::pystr::PyStr;
use crate::pyvalue::{DictKey, Opaque, PyDict, PyValue};
use agent_runtime_support::pystr::PyStr as QStr;
use agent_runtime_support::pyvalue::PyValue as QValue;
use role_contract::load::LoadFailure;
use std::fmt::Debug;
use std::sync::Arc;

/// Exception class reported when the role bridge cannot answer: a situation outside the library's profile, a query on
/// the wrong kind of value, a self-referencing value that is materialised.
pub const BRIDGE_ERROR: &str = "RoleContractBridgeError";

/// An exception of the role owner: its Python class name and `str`.
#[derive(Clone, Debug)]
pub struct RoleFailure {
    pub class: &'static str,
    pub message: PyStr,
}

impl RoleFailure {
    pub fn new(class: &'static str, message: &str) -> Self {
        RoleFailure {
            class,
            message: PyStr::new(message),
        }
    }
}

/// The two operations of the owner the classifier asks for. `load_yaml` answers the document, `evaluate_manifest` the
/// result for a document and a topology (with the owner's default `role`).
pub trait RoleSource: Send + Sync + Debug {
    fn load_yaml(&self, path: &PyStr) -> Result<QValue, RoleFailure>;
    fn evaluate_manifest(&self, document: &QValue, topology: &str) -> Result<QValue, RoleFailure>;
}

/// The production source: the `role-contract` library itself.
#[derive(Debug)]
pub struct FixedRoleContract;

fn bridge(message: &str) -> RoleFailure {
    RoleFailure::new(BRIDGE_ERROR, message)
}

fn errno_class(errno: i32) -> &'static str {
    match errno {
        2 => "FileNotFoundError",
        13 => "PermissionError",
        20 => "NotADirectoryError",
        21 => "IsADirectoryError",
        _ => "OSError",
    }
}

fn failure_of(failure: LoadFailure) -> RoleFailure {
    match failure {
        LoadFailure::NotPathLike(name) => RoleFailure::new(
            "TypeError",
            &format!("expected str, bytes or os.PathLike object, not {name}"),
        ),
        LoadFailure::EmbeddedNull => RoleFailure::new("ValueError", "embedded null byte"),
        LoadFailure::Os { error, filename } => RoleFailure {
            class: errno_class(error.raw_os_error().unwrap_or(0)),
            message: PyStr::new(&crate::cli::os_error_text(
                &error,
                &PyStr::from_code_points(filename.code_points().to_vec()),
            )),
        },
        LoadFailure::Yaml(error) => RoleFailure {
            class: error.class,
            message: PyStr::from_code_points(error.message.code_points().to_vec()),
        },
        LoadFailure::Unsupported { boundary, detail } => bridge(&format!(
            "outside the role-contract profile ({boundary}): {detail}"
        )),
    }
}

impl RoleSource for FixedRoleContract {
    fn load_yaml(&self, path: &PyStr) -> Result<QValue, RoleFailure> {
        let path = QValue::Str(QStr::from_code_points(path.code_points().to_vec()));
        role_contract::load::load_yaml(&path).map_err(failure_of)
    }

    fn evaluate_manifest(&self, document: &QValue, topology: &str) -> Result<QValue, RoleFailure> {
        Ok(role_contract::evaluate_manifest(
            document,
            &QValue::str(topology),
            &QValue::str(role_contract::SUB_ROLE),
        )
        .to_value())
    }
}

/// One step from a value to a value inside it.
#[derive(Clone, Debug)]
enum Step {
    Key(String),
    Index(usize),
}

/// A value inside a document the role library holds: the shared root and the way down to it. Cloning a node clones the
/// pointer and the path, never the document.
#[derive(Clone, Debug)]
pub struct Node {
    root: Arc<QValue>,
    path: Vec<Step>,
}

impl Node {
    fn value(&self) -> Option<&QValue> {
        let mut current = self.root.peel();
        for step in &self.path {
            current = match (step, current) {
                (Step::Key(key), QValue::Dict(dict)) => dict.get(key)?.peel(),
                (Step::Index(at), QValue::List(items) | QValue::Tuple(items)) => {
                    items.get(*at)?.peel()
                }
                _ => return None,
            };
        }
        Some(current)
    }

    fn child(&self, step: Step) -> Node {
        let mut path = self.path.clone();
        path.push(step);
        Node {
            root: self.root.clone(),
            path,
        }
    }
}

fn text(value: &QStr) -> PyStr {
    PyStr::from_code_points(value.code_points().to_vec())
}

fn is_cycle_marker(opaque: &Opaque) -> bool {
    matches!(
        (opaque.type_name, opaque.repr.as_str()),
        ("list", "[...]") | ("dict", "{...}")
    )
}

fn opaque(value: &Opaque) -> PyValue {
    // One nominal value type since plan_26111708 A2: the library value is this crate's value, kept whole.
    PyValue::Opaque(value.clone())
}

/// A scalar of the library's value family as this crate's value (containers are handles, never passed here).
fn scalar(value: &QValue) -> PyValue {
    match value.peel() {
        QValue::None => PyValue::None,
        QValue::Bool(flag) => PyValue::Bool(*flag),
        QValue::Int(number) => PyValue::Int(number.clone()),
        QValue::Float(number) => PyValue::Float(*number),
        QValue::Str(string) => PyValue::Str(text(string)),
        QValue::Bytes(bytes) => PyValue::Bytes(bytes.clone()),
        QValue::Opaque(inner) => opaque(inner),
        // containers are described, not converted; an empty stand-in only keeps the function total
        QValue::List(_) => PyValue::List(Vec::new()),
        QValue::Tuple(_) => PyValue::Tuple(Vec::new()),
        QValue::Dict(_) => PyValue::Dict(PyDict::new()),
        QValue::Shared(inner) => scalar(inner),
    }
}

/// The whole value, for the few places that need its content (a tuple used as a node role). A self-referencing value
/// cannot be materialised and is reported on demand as a bridge failure.
fn materialize_value(value: &QValue) -> Result<PyValue, RoleFailure> {
    Ok(match value.peel() {
        QValue::List(items) => PyValue::List(
            items
                .iter()
                .map(materialize_value)
                .collect::<Result<_, _>>()?,
        ),
        QValue::Tuple(items) => PyValue::Tuple(
            items
                .iter()
                .map(materialize_value)
                .collect::<Result<_, _>>()?,
        ),
        QValue::Dict(dict) => {
            let mut out = PyDict::new();
            for (other, key, item) in dict.keyed_entries() {
                let item = materialize_value(item)?;
                match other {
                    None => out.insert(text(key), item),
                    Some(other) => out.insert_key(
                        DictKey::Other(other.clone()),
                        item,
                    ),
                }
            }
            PyValue::Dict(out)
        }
        QValue::Opaque(inner) if is_cycle_marker(inner) => {
            return Err(bridge(
                "a value that contains itself cannot be materialised (the owner's process held it; this bridge refuses on demand)",
            ))
        }
        other => scalar(other),
    })
}

/// The descriptor of the value at `node`.
fn describe(node: Node) -> Result<Descriptor, RoleFailure> {
    let Some(value) = node.value() else {
        return Err(bridge(
            "a path of the held document no longer leads to a value",
        ));
    };
    Ok(match value {
        QValue::List(items) => Descriptor::Container {
            kind: "list",
            len: items.len(),
            node,
        },
        QValue::Tuple(items) => Descriptor::Container {
            kind: "tuple",
            len: items.len(),
            node,
        },
        QValue::Dict(dict) => Descriptor::Container {
            kind: "dict",
            len: dict.len(),
            node,
        },
        other => Descriptor::Value(scalar(other)),
    })
}

fn root_descriptor(root: &Arc<QValue>) -> Result<Descriptor, RoleFailure> {
    describe(Node {
        root: root.clone(),
        path: Vec::new(),
    })
}

fn failure(failure: RoleFailure) -> Failure {
    Failure::with_message(failure.class, failure.message)
}

/// A loaded role owner: the queries of the original's code against `node_role_contract`. The outer `Err` of a method is
/// a failure of the bridge; the inner `Err` of [`Raised`] is an exception the owner itself raised (class and `str`).
pub struct NativeRole {
    source: Arc<dyn RoleSource>,
    document: Option<Arc<QValue>>,
}

impl NativeRole {
    pub fn new(source: Arc<dyn RoleSource>) -> Self {
        NativeRole {
            source,
            document: None,
        }
    }

    fn raised(result: Result<Arc<QValue>, RoleFailure>) -> Result<Raised<Descriptor>, Failure> {
        match result {
            Ok(root) => root_descriptor(&root).map(Ok).map_err(failure),
            Err(owner) => Ok(Err(failure(owner))),
        }
    }

    /// `contract.load_yaml(manifest_path)`: the document, held as one shared value.
    pub fn load_yaml(&mut self, manifest_path: &PyStr) -> Result<Raised<Descriptor>, Failure> {
        let loaded = self.source.load_yaml(manifest_path).map(Arc::new);
        if let Ok(root) = &loaded {
            self.document = Some(root.clone());
        }
        Self::raised(loaded)
    }

    /// `contract.evaluate_manifest(doc, topology=topology)` on the document the last `load_yaml` returned.
    pub fn evaluate_manifest(&mut self, topology: &str) -> Result<Raised<Descriptor>, Failure> {
        let Some(document) = self.document.clone() else {
            return Err(failure(bridge(
                "evaluate_manifest asked before a document was loaded",
            )));
        };
        Self::raised(
            self.source
                .evaluate_manifest(&document, topology)
                .map(Arc::new),
        )
    }

    /// `value.get(key)` of a dict the owner holds (`None` when the key is absent).
    pub fn get(&mut self, value: &Descriptor, key: &str) -> Result<Option<Descriptor>, Failure> {
        let Descriptor::Container { node, kind, .. } = value else {
            return Err(failure(bridge(
                "get asked of a value that is not a container",
            )));
        };
        if *kind != "dict" {
            return Err(failure(bridge(&format!(
                "get asked of a {kind}, not a dict"
            ))));
        }
        let Some(QValue::Dict(dict)) = node.value() else {
            return Err(failure(bridge("a held dict is no longer a dict")));
        };
        if dict.get(key).is_none() {
            return Ok(None);
        }
        describe(node.child(Step::Key(key.to_string())))
            .map(Some)
            .map_err(failure)
    }

    /// `for item in value`: a list's or tuple's elements, a dict's keys, as descriptors.
    pub fn items(&mut self, value: &Descriptor) -> Result<Vec<Descriptor>, Failure> {
        match value {
            Descriptor::Value(scalar) => crate::pydyn::iterate(scalar)
                .map(|items| items.into_iter().map(Descriptor::Value).collect()),
            Descriptor::Container { node, .. } => match node.value() {
                Some(QValue::List(items) | QValue::Tuple(items)) => (0..items.len())
                    .map(|at| describe(node.child(Step::Index(at))).map_err(failure))
                    .collect(),
                Some(QValue::Dict(dict)) => dict
                    .keyed_entries()
                    .map(|(other, key, _)| match other {
                        None => Ok(Descriptor::Value(PyValue::Str(text(key)))),
                        Some(other) => match other.value.as_deref() {
                            Some(held) => Ok(Descriptor::Value(scalar(held))),
                            None => Err(failure(bridge(
                                "a dict key of a kind the document did not keep",
                            ))),
                        },
                    })
                    .collect(),
                _ => Err(failure(bridge("a held container is no longer a container"))),
            },
        }
    }

    /// The whole value as a [`PyValue`], for the few places that need its content (a tuple used as a node role).
    pub fn materialize(&mut self, value: &Descriptor) -> Result<PyValue, Failure> {
        match value {
            Descriptor::Value(scalar) => Ok(scalar.clone()),
            Descriptor::Container { node, .. } => {
                let Some(held) = node.value() else {
                    return Err(failure(bridge("a held value is gone")));
                };
                materialize_value(held).map_err(failure)
            }
        }
    }
}
