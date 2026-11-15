//! The Draft-07 subset schema validator of `completion_gate.validate_against_schema`, as data in and violations out.
//!
//! Implements exactly: local `$ref`, `type` (union, bool-vs-int/number), `required`, `properties`, `additionalProperties`
//! (`false` and schema forms), `enum`, `const`, `minimum`, `minLength`, `minItems`, `items`, `allOf`, `if/then/else`.
//! Violations are `(reason_code, message)` pairs in the order the owner produces them. A schema the owner cannot read
//! (an unsupported `type`, a non-local `$ref`, a dangling `$ref` path) is the exception it raises: [`SchemaError`].

use crate::pystr::PyStr;
use crate::pyvalue::PyValue;

/// An exception the owner raises on a schema it cannot read: Python class name and text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaError {
    pub class: &'static str,
    pub message: String,
}

pub type Violation = (String, String);
type Out = Result<Vec<Violation>, SchemaError>;

fn text(value: &PyValue) -> Option<String> {
    match value.peel() {
        PyValue::Str(s) => s.to_utf8(),
        _ => None,
    }
}

fn is_number(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Int(_) | PyValue::Float(_))
}

fn as_f64(value: &PyValue) -> Option<f64> {
    match value.peel() {
        PyValue::Int(n) => n.to_string().parse::<f64>().ok(),
        PyValue::Float(f) => Some(*f),
        _ => None,
    }
}

/// Python `==` between JSON values (`1 == 1.0`, nothing else crosses types).
fn py_eq(a: &PyValue, b: &PyValue) -> bool {
    match (a.peel(), b.peel()) {
        (PyValue::Bool(x), PyValue::Bool(y)) => x == y,
        (PyValue::Bool(x), PyValue::Int(_) | PyValue::Float(_)) | (PyValue::Int(_) | PyValue::Float(_), PyValue::Bool(x)) => {
            let other = if matches!(a.peel(), PyValue::Bool(_)) { b } else { a };
            as_f64(other) == Some(if *x { 1.0 } else { 0.0 })
        }
        (PyValue::Int(_) | PyValue::Float(_), PyValue::Int(_) | PyValue::Float(_)) => match (a.peel(), b.peel()) {
            (PyValue::Int(x), PyValue::Int(y)) => x == y,
            _ => as_f64(a) == as_f64(b),
        },
        (PyValue::List(x), PyValue::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q)),
        (PyValue::Dict(x), PyValue::Dict(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| match y.get(&k.to_utf8().unwrap_or_default()) {
                    Some(w) => py_eq(v, w),
                    None => false,
                })
        }
        (x, y) => x == y,
    }
}

fn is_bool(v: &PyValue) -> bool {
    matches!(v.peel(), PyValue::Bool(_))
}

fn type_matches(value: &PyValue, json_type: &str) -> Result<bool, SchemaError> {
    Ok(match json_type {
        "object" => matches!(value.peel(), PyValue::Dict(_)),
        "array" => matches!(value.peel(), PyValue::List(_)),
        "string" => matches!(value.peel(), PyValue::Str(_)),
        "integer" => matches!(value.peel(), PyValue::Int(_)),
        "number" => is_number(value),
        "boolean" => is_bool(value),
        "null" => matches!(value.peel(), PyValue::None),
        other => {
            return Err(SchemaError {
                class: "ValueError",
                message: format!("completion_gate: schema uses unsupported type keyword {}", PyStr::new(other).repr()),
            })
        }
    })
}

fn field(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn resolve_ref(reference: &str, root: &PyValue) -> Result<PyValue, SchemaError> {
    if !reference.starts_with("#/") {
        return Err(SchemaError {
            class: "ValueError",
            message: format!("completion_gate: only local $ref is supported, got {}", PyStr::new(reference).repr()),
        });
    }
    let mut node = root.clone();
    for part in reference[2..].split('/') {
        let next = match node.peel() {
            PyValue::Dict(d) => d.get(part).cloned(),
            _ => None,
        };
        match next {
            Some(n) => node = n,
            None => {
                return Err(SchemaError { class: "KeyError", message: PyStr::new(part).repr() });
            }
        }
    }
    Ok(node)
}

/// `validate_against_schema(instance, schema)`.
pub fn validate_against_schema(instance: &PyValue, schema: &PyValue) -> Out {
    validate(instance, schema, schema, "")
}

fn validate(instance: &PyValue, schema: &PyValue, root: &PyValue, path: &str) -> Out {
    let empty = crate::pyvalue::PyDict::new();
    let schema_dict = match schema.peel() {
        PyValue::Dict(d) => d,
        _ => &empty,
    };
    let mut violations: Vec<Violation> = Vec::new();
    let label = if path.is_empty() { "$" } else { path };
    if let Some(reference) = schema_dict.get("$ref") {
        let target = resolve_ref(&text(reference).unwrap_or_default(), root)?;
        return validate(instance, &target, root, path);
    }
    if let Some(kind) = schema_dict.get("type") {
        let allowed: PyValue = match kind.peel() {
            PyValue::List(_) => kind.clone(),
            _ => PyValue::List(vec![kind.clone()]),
        };
        let mut matched = false;
        if let PyValue::List(items) = allowed.peel() {
            for t in items {
                if type_matches(instance, &text(t).unwrap_or_default())? {
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            violations.push((
                format!("SCHEMA_TYPE_MISMATCH:{label}"),
                format!("{label}: expected type {}, got {} ({})", allowed.repr(), instance.type_name(), instance.repr()),
            ));
            return Ok(violations);
        }
    }
    if let Some(all) = schema_dict.get("allOf") {
        if let PyValue::List(subs) = all.peel() {
            for sub in subs {
                violations.extend(validate(instance, sub, root, path)?);
            }
        }
    }
    if let Some(cond) = schema_dict.get("if") {
        if validate(instance, cond, root, path)?.is_empty() {
            if let Some(then) = schema_dict.get("then") {
                violations.extend(validate(instance, then, root, path)?);
            }
        } else if let Some(other) = schema_dict.get("else") {
            violations.extend(validate(instance, other, root, path)?);
        }
    }
    if let Some(expected) = schema_dict.get("const") {
        if !py_eq(instance, expected) || is_bool(instance) != is_bool(expected) {
            violations.push((
                format!("SCHEMA_CONST_VIOLATION:{label}"),
                format!("{label}: must equal {}, got {}", expected.repr(), instance.repr()),
            ));
        }
    }
    if let Some(allowed) = schema_dict.get("enum") {
        let items: Vec<PyValue> = match allowed.peel() {
            PyValue::List(l) => l.clone(),
            _ => Vec::new(),
        };
        if !items.iter().any(|v| py_eq(instance, v) && is_bool(instance) == is_bool(v)) {
            violations.push((
                format!("SCHEMA_ENUM_VIOLATION:{label}"),
                format!("{label}: {} is not one of {}", instance.repr(), allowed.repr()),
            ));
        }
    }
    if let (PyValue::Str(s), Some(min)) = (instance.peel(), schema_dict.get("minLength")) {
        if let Some(m) = as_f64(min) {
            if (s.len() as f64) < m {
                violations.push((
                    format!("SCHEMA_MIN_LENGTH_VIOLATION:{label}"),
                    format!("{label}: length {} < minLength {}", s.len(), min.display_text().to_utf8().unwrap_or_default()),
                ));
            }
        }
    }
    if is_number(instance) {
        if let Some(min) = schema_dict.get("minimum") {
            if let (Some(i), Some(m)) = (as_f64(instance), as_f64(min)) {
                if i < m {
                    violations.push((
                        format!("SCHEMA_MINIMUM_VIOLATION:{label}"),
                        format!("{label}: {} < minimum {}", instance.repr(), min.display_text().to_utf8().unwrap_or_default()),
                    ));
                }
            }
        }
    }
    if let (PyValue::List(items), Some(min)) = (instance.peel(), schema_dict.get("minItems")) {
        if let Some(m) = as_f64(min) {
            if (items.len() as f64) < m {
                violations.push((
                    format!("SCHEMA_MIN_ITEMS_VIOLATION:{label}"),
                    format!("{label}: length {} < minItems {}", items.len(), min.display_text().to_utf8().unwrap_or_default()),
                ));
            }
        }
    }
    if let PyValue::Dict(inst) = instance.peel() {
        let none = PyValue::Dict(crate::pyvalue::PyDict::new());
        let props = schema_dict.get("properties").unwrap_or(&none);
        let props = match props.peel() {
            PyValue::Dict(d) => d.clone(),
            _ => crate::pyvalue::PyDict::new(),
        };
        if let Some(PyValue::List(req)) = schema_dict.get("required").map(|v| v.peel()) {
            for key in req {
                let key = text(key).unwrap_or_default();
                if !inst.contains_key(&key) {
                    let f = field(path, &key);
                    violations.push((format!("SCHEMA_MISSING_REQUIRED_KEY:{f}"), format!("{f}: required property is missing")));
                }
            }
        }
        let additional = schema_dict.get("additionalProperties");
        if matches!(additional.map(|v| v.peel()), Some(PyValue::Bool(false))) {
            for (key, _) in inst.iter() {
                let key = key.to_utf8().unwrap_or_default();
                if !props.contains_key(&key) {
                    let f = field(path, &key);
                    violations.push((
                        format!("SCHEMA_UNKNOWN_PROPERTY:{f}"),
                        format!("{f}: unrecognized property (additionalProperties: false)"),
                    ));
                }
            }
        }
        for (key, sub) in props.iter() {
            let key = key.to_utf8().unwrap_or_default();
            if let Some(value) = inst.get(&key) {
                violations.extend(validate(value, sub, root, &field(path, &key))?);
            }
        }
        if let Some(extra) = additional.filter(|v| matches!(v.peel(), PyValue::Dict(_))) {
            for (key, value) in inst.iter() {
                let key = key.to_utf8().unwrap_or_default();
                if !props.contains_key(&key) {
                    violations.extend(validate(value, extra, root, &field(path, &key))?);
                }
            }
        }
    }
    if let (PyValue::List(items), Some(sub)) = (instance.peel(), schema_dict.get("items")) {
        for (i, item) in items.iter().enumerate() {
            violations.extend(validate(item, sub, root, &format!("{path}[{i}]"))?);
        }
    }
    Ok(violations)
}
