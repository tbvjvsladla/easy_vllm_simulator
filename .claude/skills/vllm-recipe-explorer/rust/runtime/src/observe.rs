//! Whole-value comparison as the frozen observers make it: exact types (`bool` is not `int`, `1` is not `1.0`), object key
//! order, list order, and NaN equal to NaN. Used to check a result against a saved original result without any
//! normalisation.

use agent_runtime_support::pyvalue::PyValue;

/// True when the two values have the same types, the same key order and the same contents.
pub fn same_value(expected: &PyValue, actual: &PyValue) -> bool {
    match (expected.peel(), actual.peel()) {
        (PyValue::Float(a), PyValue::Float(b)) => (a.is_nan() && b.is_nan()) || a == b,
        (PyValue::List(a), PyValue::List(b)) | (PyValue::Tuple(a), PyValue::Tuple(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same_value(x, y))
        }
        (PyValue::Dict(a), PyValue::Dict(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && same_value(va, vb))
        }
        (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b) && a == b,
    }
}
